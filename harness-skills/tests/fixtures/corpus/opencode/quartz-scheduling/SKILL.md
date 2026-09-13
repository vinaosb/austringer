---
name: quartz-scheduling
description: "Quartz.NET background job scheduling patterns. Job design, cron expressions, failure handling, distributed locking, job chaining, job stores (RAM vs database), dependency injection, monitoring, concurrency control, misfire handling, calendar-based exclusion, job recovery after restart. USE FOR: scheduled tasks, recurring jobs, cron-based execution, background processing, job queues, batch processing, cleanup jobs, reminder systems, periodic data aggregation. DO NOT USE FOR: simple one-time fire-and-forget (use IHostedService), real-time event processing (use message queue), non-.NET scheduling."
license: MIT
metadata:
  author: community
  version: "1.0.0"
  frameworks: [".NET 8+", "Quartz.NET"]
---

# Quartz.NET Scheduling Patterns

## 1. Setup and Configuration

```bash
dotnet add package Quartz
dotnet add package Quartz.Extensions.Hosting
dotnet add package Quartz.Serialization.Json
```

> `Quartz.Extensions.Hosting` provides `AddQuartz()` + `AddQuartzHostedService()`. With only `Quartz` + `Quartz.Serialization.Json`, registration methods unavailable. Add as NuGet reference.

Register with generic host in `Program.cs`:

```csharp
builder.Services.AddQuartz(q =>
{
    q.UseMicrosoftDependencyInjectionJobFactory();

    // RAM store (default, lost on restart)
    q.UseInMemoryStore();

    // Or persistent store for production:
    // q.UsePersistentStore(s =>
    // {
    //     s.UsePostgres(connectionString);
    //     s.UseJsonSerializer();
    //     s.UseClustering(c =>
    //     {
    //         c.CheckinInterval = TimeSpan.FromSeconds(10);
    //         c.CheckinMisfireThreshold = TimeSpan.FromSeconds(20);
    //     });
    // });

    // Global job misfire threshold
    q.MisfireThreshold = TimeSpan.FromSeconds(60);
});

builder.Services.AddQuartzHostedService(options =>
{
    options.WaitForJobsToComplete = true;
    options.AwaitApplicationStarted = true;
});
```

`AddQuartzHostedService` starts scheduler on host start, shuts down gracefully. `WaitForJobsToComplete` lets running jobs finish before exit.

**`AddQuartz()` is additive.** Called from multiple module registrations merges configs into single scheduler. Do NOT call `AddQuartzHostedService()` more than once — call once after all `AddQuartz()` calls, typically in entry point.

Choose job store by deployment:

| Store | When to use |
|-------|-------------|
| RAM (in-memory) | Single-instance, dev, ephemeral |
| PostgreSQL / SQL Server / SQLite | Production clusters, durable jobs |
| Redis (community) | High-throughput, Redis already running |

## 2. Job Design Patterns

Every job implements `IJob` with single `Execute`:

```csharp
public class OrderReminderJob : IJob
{
    private readonly ILogger<OrderReminderJob> _logger;

    public OrderReminderJob(ILogger<OrderReminderJob> logger)
    {
        _logger = logger;
    }

    public async Task Execute(IJobExecutionContext context)
    {
        var dataMap = context.MergedJobDataMap;
        var minutesBefore = dataMap.GetInt("MinutesBefore");

        _logger.LogInformation("Sending reminders for orders due in {Minutes} minutes",
            minutesBefore);

        // Job logic here
    }
}
```

Prevent concurrent execution:

```csharp
[DisallowConcurrentExecution]
public class CleanupJob : IJob
{
    // Quartz won't fire again until current execution completes
}
```

Job identity:

```csharp
var job = JobBuilder.Create<OrderReminderJob>()
    .WithIdentity("order-reminder-30min", "orders")
    .WithDescription("Sends reminder 30 minutes before order delivery")
    .UsingJobData("MinutesBefore", 30)
    .UsingJobData("MaxRetries", 3)
    .StoreDurably()                         // survive without trigger
    .RequestRecovery(true)                  // re-execute after crash
    .Build();
```

Group jobs logically: `"orders"`, `"billing"`, `"cleanup"`, `"reports"`. Makes pause/query per group easy.

## 3. Cron Expressions

Seven fields: seconds, minutes, hours, day-of-month, month, day-of-week, year.

```
┌───── second (0-59)
│ ┌───── minute (0-59)
│ │ ┌───── hour (0-23)
│ │ │ ┌───── day of month (1-31, or L for last)
│ │ │ │ ┌───── month (1-12, or JAN-DEC)
│ │ │ │ │ ┌───── day of week (1-7, MON-SUN, or L, #, ?)
│ │ │ │ │ │ ┌───── year (optional, 1970-2099)
│ │ │ │ │ │ │
* * * * * ? *
```

| Schedule | Cron |
|----------|------|
| Every 5 min | `0 0/5 * * * ? *` |
| Every 30s | `0/30 * * * * ? *` |
| Daily 2AM | `0 0 2 * * ? *` |
| Weekdays 9AM | `0 0 9 ? * MON-FRI *` |
| Last day of month midnight | `0 0 0 L * ? *` |
| First Monday each month | `0 0 0 ? * 2#1 *` |
| Mon+Wed 3:30PM | `0 30 15 ? * MON,WED *` |
| Hourly 9AM-5PM weekdays | `0 0 9-17 ? * MON-FRI *` |

Validate at startup:

```csharp
var expression = new CronExpression("0 0/5 * * * ? *");
if (!expression.IsValidExpression())
{
    throw new InvalidOperationException("Invalid cron expression");
}
```

## 4. Dependency Injection in Jobs

Singleton services (loggers, options, `IHttpClientFactory`) injectable directly. Scoped services (`DbContext`, `IDistributedCache`, `IClock`) cannot — use `IServiceScopeFactory`:

```csharp
public class DataAggregationJob : IJob
{
    private readonly IServiceScopeFactory _scopeFactory;
    private readonly ILogger<DataAggregationJob> _logger;

    public DataAggregationJob(
        IServiceScopeFactory scopeFactory,
        ILogger<DataAggregationJob> logger)
    {
        _scopeFactory = scopeFactory;
        _logger = logger;
    }

    public async Task Execute(IJobExecutionContext context)
    {
        await using var scope = _scopeFactory.CreateAsyncScope();
        var dbContext = scope.ServiceProvider.GetRequiredService<AppDbContext>();
        var repository = scope.ServiceProvider.GetRequiredService<IReportRepository>();

        var cutoff = DateTime.UtcNow.AddDays(-30);
        var report = await repository.BuildMonthlyReportAsync(cutoff);
        await dbContext.SaveChangesAsync(context.CancellationToken);
    }
}
```

Jobs resolved from DI via `UseMicrosoftDependencyInjectionJobFactory()`. Jobs created per execution, not cached as singletons.

## 5. Failure Handling

```csharp
public class SyncJob : IJob
{
    public async Task Execute(IJobExecutionContext context)
    {
        try
        {
            var result = await DoSyncAsync(context.CancellationToken);
            context.Result = $"Processed {result.Count} records";
        }
        catch (Exception ex) when (ex is not OperationCanceledException)
        {
            _logger.LogError(ex, "Sync job failed");
            throw new JobExecutionException(msg: "Sync failed, retrying", refireImmediately: false);
        }
    }
}
```

`JobExecutionException` controls behavior: `refireImmediately: true` retries immediately (sparingly), `false` lets misfire policy decide.

Track attempts in data map:

```csharp
public async Task Execute(IJobExecutionContext context)
{
    var attempts = context.MergedJobDataMap.GetInt("Attempts") + 1;
    context.MergedJobDataMap.Put("Attempts", attempts);

    if (attempts > MaxAttempts)
    {
        _logger.LogError("Job failed after {Attempts} attempts, moving to dead letter", attempts);
        await MoveToDeadLetterQueueAsync(context);
        return;
    }

    try { await DoWorkAsync(); }
    catch (Exception ex) { throw new JobExecutionException("Retrying", refireImmediately: false); }
}
```

## 6. Misfire Handling

Misfire occurs when scheduled fire time missed (server down, thread pool exhausted, job ran too long). Configure on trigger:

```csharp
var trigger = TriggerBuilder.Create()
    .WithIdentity("order-reminder-30min-trigger", "orders")
    .ForJob("order-reminder-30min", "orders")
    .WithCronSchedule("0 0/5 * * * ? *", x =>
    {
        x.WithMisfireHandlingInstructionFireAndProceed();
        // x.WithMisfireHandlingInstructionIgnoreMisfires();    // fire ALL missed
        // x.WithMisfireHandlingInstructionDoNothing();         // wait for next
    })
    .Build();
```

| Policy | Behavior |
|--------|----------|
| `IgnoreMisfires` | Fire every missed immediately |
| `FireAndProceed` | Fire first missed, discard rest |
| `DoNothing` | Wait for next scheduled time |
| `SmartPolicy` | Quartz picks by trigger type (recommended default) |

Reminders: `FireAndProceed` (user gets at least one). Idempotent aggregation: `IgnoreMisfires` (no data skipped). Time-sensitive: `DoNothing` (avoid wasted work).

## 7. Distributed Locking

Quartz handles natively with persistent job store + clustering:

```csharp
q.UsePersistentStore(s =>
{
    s.UsePostgres(connectionString);
    s.UseJsonSerializer();
    s.UseClustering(c =>
    {
        c.CheckinInterval = TimeSpan.FromSeconds(15);
    });
});
```

Add `[DisallowConcurrentExecution]` as second layer — prevents same node firing twice.

Without clustering, use distributed lock provider:

```csharp
public async Task Execute(IJobExecutionContext context)
{
    await using var handle = await _distributedLockProvider.AcquireLockAsync(
        "cleanup-job-lock", TimeSpan.FromMinutes(5));

    if (handle == null)
    {
        _logger.LogInformation("Another instance holds the lock, skipping");
        return;
    }

    await DoCleanupAsync();
}
```

## 8. Job Chaining

Use `JobChainingJobListener` for sequence:

```csharp
var listener = new JobChainingJobListener("report-chain");
listener.AddJobChainLink("step1-collect", "step2-transform");
listener.AddJobChainLink("step2-transform", "step3-publish");

q.SetGlobalJobListener(listener);
```

Conditional chaining (on success only):

```csharp
public class ConditionalChainListener : JobListenerSupport
{
    private readonly Dictionary<string, string> _onSuccess = new();
    private readonly ISchedulerFactory _schedulerFactory;

    public override string Name => "conditional-chain";

    public void ChainOnSuccess(string fromJob, string toJob)
        => _onSuccess[fromJob] = toJob;

    public override Task JobWasExecuted(
        IJobExecutionContext context,
        JobExecutionException? jobException,
        CancellationToken cancellationToken)
    {
        if (jobException != null)
        {
            _logger.LogWarning("Job {Job} failed, skipping chain", context.JobDetail.Key);
            return Task.CompletedTask;
        }

        if (_onSuccess.TryGetValue(context.JobDetail.Key.Name, out var nextJob))
        {
            var scheduler = _schedulerFactory.GetScheduler(cancellationToken)
                .GetAwaiter().GetResult();
            return scheduler.TriggerJob(
                JobKey.Create(nextJob, context.JobDetail.Key.Group),
                cancellationToken: cancellationToken);
        }

        return Task.CompletedTask;
    }
}
```

## 9. Calendar-Based Scheduling

Exclude holidays, weekends, maintenance windows:

```csharp
var holidayCalendar = new HolidayCalendar();

holidayCalendar.AddExcludedDate(new DateTime(2026, 12, 25));
holidayCalendar.AddExcludedDate(new DateTime(2026, 1, 1));

var weeklyCalendar = new WeeklyCalendar();
weeklyCalendar.SetDayExcluded(DayOfWeek.Saturday, true);
weeklyCalendar.SetDayExcluded(DayOfWeek.Sunday, true);

await scheduler.AddCalendar("holidays", holidayCalendar, false, true);
await scheduler.AddCalendar("weekdays", weeklyCalendar, false, true);

var trigger = TriggerBuilder.Create()
    .WithCronSchedule("0 0 9 ? * MON-FRI *")
    .ModifiedByCalendar("holidays")
    .Build();
```

`DailyCalendar` blocks time ranges (e.g., 2AM-4AM maintenance):

```csharp
var maintenanceCalendar = new DailyCalendar("02:00:00", "04:00:00");
await scheduler.AddCalendar("maintenance-window", maintenanceCalendar, false, true);
```

## 10. Monitoring and Management

Listeners for observability:

```csharp
public class JobMetricsListener : JobListenerSupport
{
    private readonly ILogger<JobMetricsListener> _logger;

    public override string Name => "job-metrics";

    public override Task JobToBeExecuted(
        IJobExecutionContext context,
        CancellationToken cancellationToken)
    {
        context.Put("StartTime", DateTime.UtcNow);
        return Task.CompletedTask;
    }

    public override Task JobWasExecuted(
        IJobExecutionContext context,
        JobExecutionException? jobException,
        CancellationToken cancellationToken)
    {
        var start = (DateTime)context.Get("StartTime")!;
        var duration = DateTime.UtcNow - start;
        var status = jobException != null ? "FAILED" : "OK";

        _logger.LogInformation(
            "Job {Job} [{Group}] {Status} in {DurationMs}ms",
            context.JobDetail.Key.Name,
            context.JobDetail.Key.Group,
            status,
            duration.TotalMilliseconds);

        return Task.CompletedTask;
    }
}
```

Query job state at runtime:

```csharp
public class JobManagementService
{
    private readonly ISchedulerFactory _schedulerFactory;

    public async Task<IReadOnlyList<JobStatusDto>> GetAllJobsAsync()
    {
        var scheduler = await _schedulerFactory.GetScheduler();
        var jobGroups = await scheduler.GetJobGroupNames();
        var result = new List<JobStatusDto>();

        foreach (var group in jobGroups)
        {
            var keys = await scheduler.GetJobKeys(GroupMatcher<JobKey>.GroupEquals(group));
            foreach (var key in keys)
            {
                var detail = await scheduler.GetJobDetail(key);
                var triggers = await scheduler.GetTriggersOfJob(key);
                foreach (var trigger in triggers)
                {
                    var state = await scheduler.GetTriggerState(trigger.Key);
                    result.Add(new JobStatusDto
                    {
                        JobName = key.Name,
                        Group = group,
                        TriggerState = state.ToString(),
                        NextFireTime = trigger.GetNextFireTimeUtc(),
                        PreviousFireTime = trigger.GetPreviousFireTimeUtc()
                    });
                }
            }
        }

        return result;
    }

    public async Task PauseJobAsync(string jobName, string group)
    {
        var scheduler = await _schedulerFactory.GetScheduler();
        await scheduler.PauseJob(JobKey.Create(jobName, group));
    }

    public async Task TriggerJobManuallyAsync(string jobName, string group)
    {
        var scheduler = await _schedulerFactory.GetScheduler();
        await scheduler.TriggerJob(JobKey.Create(jobName, group));
    }
}
```

## 11. Testing

Unit test by mocking dependencies:

```csharp
public class CleanupJobTests
{
    [Fact]
    public async Task Execute_ShouldDeleteRecordsOlderThanCutoff()
    {
        var repository = new Mock<IRepository>();
        repository.Setup(r => r.DeleteOlderThanAsync(It.IsAny<DateTime>()))
            .ReturnsAsync(42);

        var logger = new Mock<ILogger<CleanupJob>>();
        var job = new CleanupJob(repository.Object, logger.Object);

        var context = new Mock<IJobExecutionContext>();
        context.Setup(c => c.MergedJobDataMap)
            .Returns(new JobDataMap { { "RetentionDays", 90 } });
        context.Setup(c => c.CancellationToken).Returns(CancellationToken.None);

        await job.Execute(context.Object);

        repository.Verify(r => r.DeleteOlderThanAsync(
            It.Is<DateTime>(d => d < DateTime.UtcNow)), Times.Once);
    }
}
```

Test cron expressions:

```csharp
[Theory]
[InlineData("0 0 2 * * ? *", true)]
[InlineData("0 0/5 * * * ? *", true)]
[InlineData("invalid", false)]
public void CronExpression_ShouldValidate(string expression, bool expectedValid)
{
    var isValid = CronExpression.IsValidExpression(expression);
    isValid.ShouldBe(expectedValid);
}
```

Integration test with in-memory scheduler:

```csharp
public class JobSchedulerTests : IDisposable
{
    private readonly IScheduler _scheduler;

    public JobSchedulerTests()
    {
        var factory = new StdSchedulerFactory();
        _scheduler = factory.GetScheduler().GetAwaiter().GetResult();
        _scheduler.Start().GetAwaiter().GetResult();
    }

    [Fact]
    public async Task ScheduledJob_ShouldFire_WhenTriggerTimeArrives()
    {
        var fired = new TaskCompletionSource();
        var listener = new TestJobListener(_ => fired.SetResult());

        _scheduler.ListenerManager.AddJobListener(listener);

        var job = JobBuilder.Create<NoOpJob>()
            .WithIdentity("test-job")
            .Build();

        var trigger = TriggerBuilder.Create()
            .StartNow()
            .WithSimpleSchedule(s => s
                .WithRepeatCount(0)
                .WithIntervalInSeconds(1))
            .Build();

        await _scheduler.ScheduleJob(job, trigger);
        var completed = await Task.WhenAny(fired.Task, Task.Delay(5000));
        completed.ShouldBe(fired.Task);
    }

    public void Dispose() => _scheduler.Shutdown().GetAwaiter().GetResult();
}
```

## 12. Anti-Patterns

**Don't inject scoped services directly into job constructor.** Jobs managed by Quartz, not DI scope lifecycle. Use `IServiceScopeFactory`.

```csharp
// Wrong
public class BadJob : IJob
{
    public BadJob(AppDbContext db) { }  // captive dependency
}

// Correct
public class GoodJob : IJob
{
    private readonly IServiceScopeFactory _scopeFactory;
    public GoodJob(IServiceScopeFactory scopeFactory) { }

    public async Task Execute(IJobExecutionContext context)
    {
        await using var scope = _scopeFactory.CreateAsyncScope();
        var db = scope.ServiceProvider.GetRequiredService<AppDbContext>();
    }
}
```

**Don't use RAM store in production clusters.** Restart loses all job/trigger state. Use persistent store.

**Don't ignore misfire handling.** Default smart policy works for many cases. Cleanup with `IgnoreMisfires` fires hundreds after outage. Reminder with `DoNothing` silently skips notifications.

**Don't run long jobs without timeout.** Break work into chunks or set timeout with checkpoint:

```csharp
using var cts = CancellationTokenSource.CreateLinkedTokenSource(context.CancellationToken);
cts.CancelAfter(TimeSpan.FromMinutes(10));
```

**Don't fire-and-forget without error handling.** Every job needs try/catch. Unhandled exceptions mark execution as failed, affecting misfire behavior + recovery.

**Don't put business logic in trigger configuration.** Trigger decides when to fire. Job decides what to do.

**Don't schedule thousands of one-off jobs.** Each job + trigger adds overhead. Use single job reading from queue/table.

## Related Skills

- **dotnet-clean-arch** — Project structure (jobs in Infrastructure/Jobs/)
- **postgresql-efcore** — IDbContextFactory pattern for DB access from jobs
- **aspnetcore-health** — Health checks monitoring scheduler + job status
- **gdpr-data-privacy** — Key rotation and data retention cleanup jobs