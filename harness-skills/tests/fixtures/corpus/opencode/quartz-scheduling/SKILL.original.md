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

Install the core package and the DI integration:

```bash
dotnet add package Quartz
dotnet add package Quartz.Extensions.Hosting
dotnet add package Quartz.Serialization.Json
```

> `Quartz.Extensions.Hosting` provides both `AddQuartz()` and `AddQuartzHostedService()` extension methods. If you only have `Quartz` + `Quartz.Serialization.Json` but are missing `Quartz.Extensions.Hosting`, the registration methods won't be available. Add it as a NuGet reference.

Register Quartz with the generic host in `Program.cs`:

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

The `AddQuartzHostedService` call starts the scheduler when the host starts and shuts it down gracefully on application stop. `WaitForJobsToComplete` tells the scheduler to let running jobs finish before the process exits.

**`AddQuartz()` is additive.** Calling it from multiple module registrations (e.g., `ReportingModuleRegistration`, `DataRetentionServiceRegistration`) merges all job configurations into a single scheduler. Do NOT call `AddQuartzHostedService()` more than once — it should be called exactly once, after all `AddQuartz()` calls, typically in the application entry point.

Choose your job store based on deployment:

| Store | When to use |
|-------|-------------|
| RAM (in-memory) | Single-instance, dev, ephemeral workloads |
| PostgreSQL / SQL Server / SQLite | Production clusters, durable jobs |
| Redis (community package) | High-throughput, already running Redis |

## 2. Job Design Patterns

Every job implements `IJob` with a single `Execute` method:

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

Prevent concurrent execution of the same job:

```csharp
[DisallowConcurrentExecution]
public class CleanupJob : IJob
{
    // Quartz will not fire this job again until the current
    // execution completes, even if the trigger fires.
}
```

Key points about job identity:

```csharp
var job = JobBuilder.Create<OrderReminderJob>()
    .WithIdentity("order-reminder-30min", "orders")
    .WithDescription("Sends reminder 30 minutes before order delivery")
    .UsingJobData("MinutesBefore", 30)        // parameter
    .UsingJobData("MaxRetries", 3)
    .StoreDurably()                            // survive without a trigger
    .RequestRecovery(true)                     // re-execute after crash
    .Build();
```

Group jobs logically. Use group names like `"orders"`, `"billing"`, `"cleanup"`, `"reports"`. This makes it easy to pause or query an entire group.

## 3. Cron Expressions

A Quartz cron trigger has seven fields: seconds, minutes, hours, day-of-month, month, day-of-week, year.

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

Common patterns:

| Schedule | Cron Expression |
|----------|----------------|
| Every 5 minutes | `0 0/5 * * * ? *` |
| Every 30 seconds | `0/30 * * * * ? *` |
| Daily at 2:00 AM | `0 0 2 * * ? *` |
| Weekdays at 9:00 AM | `0 0 9 ? * MON-FRI *` |
| Last day of month at midnight | `0 0 0 L * ? *` |
| First Monday of each month | `0 0 0 ? * 2#1 *` |
| Every Monday and Wednesday at 3:30 PM | `0 30 15 ? * MON,WED *` |
| Every hour between 9 AM and 5 PM | `0 0 9-17 ? * MON-FRI *` |

Validate expressions at startup:

```csharp
var expression = new CronExpression("0 0/5 * * * ? *");
if (!expression.IsValidExpression())
{
    throw new InvalidOperationException("Invalid cron expression");
}
```

Or validate when building the trigger. Quartz throws a `SchedulerException` for invalid expressions, so wrap trigger creation in a try/catch during configuration.

## 4. Dependency Injection in Jobs

Singleton services (loggers, options, `IHttpClientFactory`) can be injected directly into the job constructor. Scoped services like `DbContext`, `IDistributedCache`, or `IClock` cannot. Use `IServiceScopeFactory` instead — the job MUST NOT inject scoped services directly:

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

`Microsoft.Extensions.DependencyInjection.Quartz` is already configured via `UseMicrosoftDependencyInjectionJobFactory()`. This lets Quartz resolve job instances from DI. Jobs are created per execution, not cached as singletons.

Register jobs with DI in the Quartz config:

```csharp
q.UseMicrosoftDependencyInjectionJobFactory();
// Jobs with constructor deps are resolved automatically.
// No manual registration needed for the job classes themselves.
```

## 5. Failure Handling

Wrap job logic in try/catch and use the job execution result to signal outcomes:

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

`JobExecutionException` controls Quartz behavior. Set `refireImmediately: true` to retry immediately (use sparingly). Set `refireImmediately: false` to let the misfire policy decide.

For retry limits, track attempts in the job data map:

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

A misfire happens when a scheduled fire time is missed (server was down, thread pool exhausted, job ran too long). Configure the policy on the trigger:

```csharp
var trigger = TriggerBuilder.Create()
    .WithIdentity("order-reminder-30min-trigger", "orders")
    .ForJob("order-reminder-30min", "orders")
    .WithCronSchedule("0 0/5 * * * ? *", x =>
    {
        x.WithMisfireHandlingInstructionFireAndProceed();  // smart policy
        // x.WithMisfireHandlingInstructionIgnoreMisfires();   // fire ALL missed
        // x.WithMisfireHandlingInstructionDoNothing();        // wait for next
    })
    .Build();
```

Policy choices:

| Policy | Behavior |
|--------|----------|
| `IgnoreMisfires` | Fire every missed execution immediately |
| `FireAndProceed` | Fire the first missed, discard the rest |
| `DoNothing` | Wait for the next scheduled fire time |
| `SmartPolicy` | Quartz picks based on trigger type (recommended default) |

For reminder jobs, use `FireAndProceed` so the user gets at least one notification. For idempotent aggregation jobs, `IgnoreMisfires` ensures no data is skipped. For jobs that lose relevance over time (daily cleanup), `DoNothing` avoids wasted work.

## 7. Distributed Locking

Quartz handles this natively when using a persistent job store with clustering enabled. Two instances won't run the same job at the same time.

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

Add `[DisallowConcurrentExecution]` on the job class as a second layer of protection. This prevents the same node from firing the job twice if the trigger overlaps with a long-running execution.

If you need locking without Quartz clustering (e.g., you use RAM store but run multiple instances), use a distributed lock provider:

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

Use `JobChainingJobListener` to run jobs in sequence:

```csharp
var listener = new JobChainingJobListener("report-chain");
listener.AddJobChainLink("step1-collect", "step2-transform");
listener.AddJobChainLink("step2-transform", "step3-publish");

// Register the listener
q.SetGlobalJobListener(listener);
```

For conditional chaining (only proceed on success), write a custom listener:

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

Exclude holidays, weekends, or maintenance windows from a trigger:

```csharp
var holidayCalendar = new HolidayCalendar();

// Add specific dates
holidayCalendar.AddExcludedDate(new DateTime(2026, 12, 25));
holidayCalendar.AddExcludedDate(new DateTime(2026, 1, 1));

// Exclude all weekends
var weeklyCalendar = new WeeklyCalendar();
weeklyCalendar.SetDayExcluded(DayOfWeek.Saturday, true);
weeklyCalendar.SetDayExcluded(DayOfWeek.Sunday, true);

// Combine multiple calendars
var combinedCalendar = new AnnualCalendar();
combinedCalendar.CalendarBase = holidayCalendar;

await scheduler.AddCalendar("holidays", holidayCalendar, false, true);
await scheduler.AddCalendar("weekdays", weeklyCalendar, false, true);

// Apply to trigger
var trigger = TriggerBuilder.Create()
    .WithCronSchedule("0 0 9 ? * MON-FRI *")
    .ModifiedByCalendar("holidays")
    .Build();
```

Use `DailyCalendar` to block specific time ranges (e.g., a maintenance window from 2 AM to 4 AM):

```csharp
var maintenanceCalendar = new DailyCalendar("02:00:00", "04:00:00");
await scheduler.AddCalendar("maintenance-window", maintenanceCalendar, false, true);
```

## 10. Monitoring and Management

Implement listeners for observability:

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

Unit test a job by mocking its dependencies:

```csharp
public class CleanupJobTests
{
    [Fact]
    public async Task Execute_ShouldDeleteRecordsOlderThanCutoff()
    {
        // Arrange
        var repository = new Mock<IRepository>();
        repository.Setup(r => r.DeleteOlderThanAsync(It.IsAny<DateTime>()))
            .ReturnsAsync(42);

        var logger = new Mock<ILogger<CleanupJob>>();
        var job = new CleanupJob(repository.Object, logger.Object);

        var context = new Mock<IJobExecutionContext>();
        context.Setup(c => c.MergedJobDataMap)
            .Returns(new JobDataMap { { "RetentionDays", 90 } });
        context.Setup(c => c.CancellationToken).Returns(CancellationToken.None);

        // Act
        await job.Execute(context.Object);

        // Assert
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

Integration test with the in-memory scheduler:

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

**Don't inject scoped services directly into the job constructor.** Jobs are managed by Quartz, not the DI container's scope lifecycle. Use `IServiceScopeFactory` to create a scope inside `Execute`.

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

**Don't use RAM store in production clusters.** When the process restarts, all job and trigger state is lost. Jobs won't recover. Use a persistent store (PostgreSQL, SQL Server, SQLite).

**Don't ignore misfire handling.** The default smart policy works for many cases, but understand what it does. A cleanup job with `IgnoreMisfires` will fire hundreds of missed executions after an outage. A reminder job with `DoNothing` will silently skip notifications.

**Don't run long jobs without a timeout.** A job that runs for hours blocks the thread pool and delays other triggers. Either break the work into chunks, or set a reasonable timeout and checkpoint progress:

```csharp
using var cts = CancellationTokenSource.CreateLinkedTokenSource(context.CancellationToken);
cts.CancelAfter(TimeSpan.FromMinutes(10));
```

**Don't fire-and-forget without error handling.** Every job should have a try/catch. Unhandled exceptions in `Execute` cause Quartz to mark the execution as failed, which affects misfire behavior and recovery. Log the exception, set a meaningful `JobExecutionException`, and handle the error path.

**Don't put business logic in the trigger configuration.** The trigger decides when to fire. The job decides what to do. If you find yourself encoding conditional logic in trigger data maps, move that logic into the job and use a simple trigger.

**Don't schedule thousands of one-off jobs.** Each job and trigger adds overhead to the scheduler. For bulk work, use a single job that reads from a queue or table, not a separate Quartz job per item.

## Related Skills

- **dotnet-clean-arch** — Project structure where jobs live in Infrastructure/Jobs/ and are registered in Program.cs
- **postgresql-efcore** — IDbContextFactory pattern for database access from jobs (see Section 4)
- **aspnetcore-health** — Health checks that monitor scheduler and job status
- **gdpr-data-privacy** — Key rotation and data retention cleanup jobs
