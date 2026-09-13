---
name: lgpd-data-privacy  
description: "LGPD (Lei Geral de Proteção de Dados) data privacy patterns for Brazilian SaaS. Consent lifecycle, multi-tenant consent management, data subject rights, breach notification, field-level encryption, key rotation, audit trails, data portability, schema-per-tenant data isolation. USE FOR: LGPD compliance, consent management, data deletion/erasure, anonymization, breach notification, encryption at rest, key rotation, audit trail, data portability export, DPO obligations, privacy by design, multi-tenant data isolation, schema-per-tenant, tenant provisioning, tenant-scoped DSAR. DO NOT USE FOR: non-Brazilian privacy law (use GDPR-specific resources), frontend UI work, general backend logic without privacy context."
license: MIT
metadata:
  author: community
  version: "1.0.0"
  frameworks: [".NET", "EF Core", "PostgreSQL"]
---

# LGPD Data Privacy Patterns for Brazilian SaaS

Production-tested patterns for LGPD-compliant .NET apps with PostgreSQL. Generalized from real SaaS.

## 1. Legal Framework Overview

**Scope (Art. 3):** LGPD applies to processing in Brazil, offering goods/services to people in Brazil, or using data collected in Brazil. Foreign companies included if targeting Brazilian users.

**Legal bases (Art. 7):** Need at least one legal basis per processing activity. Ten bases: consent, legal obligation, public policy execution by public admin, research, contract execution, judicial/administrative rights exercise, life protection, health protection (by health professionals), legitimate interest, credit protection.

**Consent requirements (Art. 8):** Consent must be free, informed, unambiguous. Specific to each purpose, revocable anytime, written (or equivalent) for sensitive data. Bundled consent (one box for ten purposes) invalid.

**Sensitive data (Art. 11):** Health, biometric, political opinions, religious beliefs, philosophical beliefs, union membership, genetic data, sexual orientation. Requires explicit, specific consent or another Art. 11 basis (e.g., health data by health pros for care delivery).

**Key ANPD resolutions:**
- Breach notification within 3 business days (Resolution 15/2024)
- DPO appointment and publication guidelines
- International data transfer approval mechanisms

**Official sources:**
- Full LGPD: https://www.planalto.gov.br/ccivil_03/_ato2015-2018/2018/lei/l13709.htm
- ANPD FAQ: https://www.gov.br/anpd/pt-br/acesso-a-informacao/perguntas-frequentes/perguntas-frequentes

## 2. Consent Lifecycle

**Grant consent:** Each record captures who granted it, purpose, legal basis, timestamp, version. Store immutable snapshot. Don't update existing records; create new ones when terms change.

```csharp
public record ConsentGranted(
    Guid SubjectId,
    ConsentType Type,
    string Purpose,
    string LegalBasis,
    DateTime GrantedAt,
    int Version
);

public enum ConsentType
{
    PrivacyPolicy,
    TermsOfUse,
    SensitiveDataProcessing,
    Marketing,
    ThirdPartySharing,
    AutomatedDecisionMaking,
    HealthDataProcessing,
    BiometricProcessing,
    FinancialDataProcessing
}
```

**Withdraw consent:** Revoke by creating new record with withdrawal timestamp. Past processing remains legal. Withdrawal affects only future data use.

**Consent dependency graph:** Some consents are prerequisites. User must accept privacy policy before sensitive data consent. Enforce with dependency map, validate in grant flow.

**Guardian/minor consent (Art. 14):** Parents/legal guardians consent for minors. Store guardian's consent + minor's identity. When minor reaches adulthood, prompt for re-consent.

**Duplicate-active-consent guard:** Before granting, check no active consent of same type/version exists for subject. Prevents flood from page refreshes or race conditions.

```csharp
public class GrantConsentCommandHandler
{
    public async Task Handle(GrantConsentCommand cmd)
    {
        var hasActive = await _consentRepository.HasActiveConsent(
            cmd.SubjectId, cmd.Type, cmd.Version);

        if (hasActive)
            throw new DuplicateConsentException(cmd.Type);

        // Validate dependency graph
        foreach (var dependency in _dependencyMap.GetDependencies(cmd.Type))
        {
            if (!await _consentRepository.HasActiveConsent(cmd.SubjectId, dependency))
                throw new MissingConsentDependencyException(dependency);
        }

        var consent = Consent.Create(cmd);
        await _consentRepository.AddAsync(consent);
    }
}
```

### Multi-Tenant Consent Management

In multi-tenant SaaS, consent scoped to collecting tenant. Subject may have different consent per tenant. Every query must include tenant context.

**Tenant-scoped consent record:**

```csharp
public record ConsentGranted(
    Guid TenantId,
    Guid SubjectId,
    ConsentType Type,
    string Purpose,
    string LegalBasis,
    DateTime GrantedAt,
    int Version
);
```

`TenantId` isolates consent per tenant. DSAR filters by both `TenantId` and `SubjectId` — export, correct, or delete data for requesting tenant only.

**ConsentPurpose constants pattern:** Use constants not magic strings. Compile-time safety + discoverability:

```csharp
public static class ConsentPurpose
{
    public const string DataProcessing = "data_processing";
    public const string Marketing = "marketing";
    public const string ThirdPartySharing = "third_party_sharing";

    public static IReadOnlySet<string> All { get; } =
        new HashSet<string> { DataProcessing, Marketing, ThirdPartySharing };
}
```

Usage: `ConsentPurpose.DataProcessing` instead of `"data_processing"`. `All` set enables validation at grant time.

**Tenant-specific dependency graphs:** Tenants may configure different consent flows. Store dependency graph per tenant, not globally.

```csharp
public class TenantConsentDependencyMap
{
    private readonly IReadOnlyDictionary<Guid, Dictionary<ConsentType, ConsentType[]>> _maps;

    public ConsentType[] GetDependencies(Guid tenantId, ConsentType type)
    {
        if (_maps.TryGetValue(tenantId, out var tenantMap)
            && tenantMap.TryGetValue(type, out var deps))
            return deps;

        return []; // No dependencies if tenant not configured
    }
}
```

**Tenant-scoped DSAR handling:**

```csharp
public class TenantScopedDsarService
{
    public async Task<DsarResult> HandleAccessRequestAsync(Guid tenantId, Guid subjectId)
    {
        var consents = await _consentRepository
            .GetActiveByTenantAndSubjectAsync(tenantId, subjectId);

        var personalData = await _subjectDataCollector
            .CollectAsync(tenantId, subjectId);

        var auditTrail = await _auditRepository
            .GetBySubjectAsync(tenantId, subjectId);

        return new DsarResult(personalData, consents, auditTrail);
    }
}
```

**Portability scoped to requesting tenant:** Export data only from tenant that received request. Exclude data from other tenants.

**Consent isolation invariant:** Consent in one tenant never interpreted as consent in another. Cross-tenant consent does not exist.

## 3. Data Subject Rights Implementation

DSAR pipeline handling all five core rights.

**Access right:** Query every entity holding personal data for subject, join with consent + audit tables. Return structured JSON.

**Correction right:** Command handler validates request, updates fields, writes audit log with old/new values.

**Anonymization/Deletion right (two-pattern approach):**

1. **Soft-delete:** `IsDeleted = true`. Never hard-delete financial, health, or regulated data. Preserves integrity for legal holds and audit.
2. **Anonymization:** Use `IAnonymizationService` to replace PII with hashed/pseudonymous values. Irreversible, appropriate after retention period expires.

```csharp
public interface IAnonymizationService
{
    Task AnonymizeAsync<TEntity>(TEntity entity) where TEntity : class;
    string AnonymizeValue(string value, string fieldName);
}
```

**Portability right:** Export all subject data in machine-readable format (JSON/CSV). Mask encrypted fields as `[PROTECTED]`. Aggregate across profile, consents, appointments, payment history, audit trail.

**SubjectDataExporter pattern:** Each domain module implements `ISubjectDataCollector`. Central exporter collects from all via DI.

```csharp
// Each module implements for its own data
public interface ISubjectDataCollector
{
    string ModuleName { get; }
    Task<IReadOnlyDictionary<string, object?>> CollectAsync(
        Guid tenantId, Guid subjectId, CancellationToken ct);
}

// Central exporter — takes all collectors via DI
public class SubjectDataExporter
{
    private readonly IEnumerable<ISubjectDataCollector> _collectors;

    public SubjectDataExporter(IEnumerable<ISubjectDataCollector> collectors)
    {
        _collectors = collectors;
    }

    public async Task<Dictionary<string, object?>> ExportAllAsync(
        Guid tenantId, Guid subjectId, CancellationToken ct)
    {
        var result = new Dictionary<string, object?>();
        foreach (var collector in _collectors)
        {
            var data = await collector.CollectAsync(tenantId, subjectId, ct);
            result[collector.ModuleName] = data;
        }
        return result;
    }
}
```

**Registration:** Register collectors as scoped. Exporter receives all via `IEnumerable<ISubjectDataCollector>` — no module-specific coupling.

```csharp
services.AddScoped<ISubjectDataCollector, PersonDataCollector>();
services.AddScoped<ISubjectDataCollector, ConsentDataCollector>();
services.AddScoped<ISubjectDataCollector, FinancialDataCollector>();
services.AddScoped<SubjectDataExporter>();
```

**Response SLA:** Immediate acknowledgment on DSAR receipt. Complete within 15 days per ANPD guidance. If more time needed, communicate reason and revised deadline.

**Evidence:** Store every DSAR as record with timestamps for receipt, completion, exported data. Creates accountability trail for ANPD.

## 4. Breach Notification Workflow

```csharp
public enum BreachStatus
{
    Detected,
    Investigating,
    DpoNotified,
    AuthorityNotified,
    SubjectsNotified,
    Resolved
}

public class BreachNotification
{
    public Guid Id { get; }
    public string ReporterId { get; }
    public string Description { get; }
    public IReadOnlyList<string> AffectedDataTypes { get; }
    public int AffectedSubjectsCount { get; }
    public RiskLevel Risk { get; }
    public BreachStatus Status { get; private set; }
    public DateTime DpoNotifiedAt { get; private set; }
    public DateTime AuthorityNotifiedAt { get; private set; }
}
```

**Timeline:** Detect → Investigate → Notify DPO immediately → Notify ANPD within 3 business days → Notify affected subjects if risk of relevant harm → Resolve + document.

**ANPD notification:** File within 3 business days of becoming aware. Include nature of data, affected subjects, measures taken, risk assessment.

**Data subject notification:** Required when breach poses risk of significant harm/serious impact.

**Incident retention:** Keep breach records at least 5 years.

**Decouple delivery from domain:** Use `IBreachNotificationService` interface so domain model doesn't depend on email/SMS/specific channel.

## 5. Field-Level Encryption

**Use-case:** Encrypt PII and sensitive fields at rest. Financial records, personal notes, OAuth tokens, chat messages never plaintext in DB.

**Pattern:** Define encryption contract in Application layer, implement in Infrastructure.

```csharp
// Application layer — no Infrastructure dependency
public interface IEncryptionService
{
    byte[] Encrypt(string plainText, string keyId);
    string Decrypt(byte[] cipherText, string keyId);
    string CurrentKeyId { get; }
}
```

**PostgreSQL pgcrypto:** Use AES-256 via pgcrypto extension. Encrypted columns are `bytea`.

**Key management:** Embed key ID alongside ciphertext. Store as `keyId:ciphertext` or structured format with version prefix.

**Dual-key fallback during rotation:** Try decrypt with current key first. If fails, try previous key. Enables zero-downtime rotation.

```csharp
public string Decrypt(byte[] cipherText, string embeddedKeyId)
{
    try
    {
        return _engine.Decrypt(cipherText, _currentKey);
    }
    catch (DecryptionException)
    {
        return _engine.Decrypt(cipherText, _previousKey);
    }
}
```

**Configuration:** Load keys via `IOptions<EncryptionSettings>`. Never store in source code or committed appsettings.json. Use env vars, secrets manager, or key vault.

### AES-256-GCM Inline Tag Embedding

AES-256-GCM auth tags embedded **inline** in byte array (last 16 bytes), NOT separate column. Simplifies schema; decryption extracts tag from ciphertext:

```csharp
// Storage format: [ciphertext (N bytes)][tag (16 bytes)]
// Total stored length = N + 16

// Decrypt — extract tag from last 16 bytes
var ciphertextData = storedBytes[..^16]; // data portion
var tag = storedBytes[^16..];            // last 16 bytes = authentication tag

// Encrypt — append tag to ciphertext
var result = new byte[ciphertext.Length + tag.Length];
Buffer.BlockCopy(ciphertext, 0, result, 0, ciphertext.Length);
Buffer.BlockCopy(tag, 0, result, ciphertext.Length, tag.Length);
```

**Rationale:** Inline tag avoids extra DB column, keeps encrypted field as single `bytea`, simplifies schema migrations and serialization. Tag always fixed-size (16 bytes for AES-256-GCM), extraction deterministic.

### AAD Format for PII Fields

AAD binds each encrypted field to specific tenant + field name, preventing field-swapping attacks:

```csharp
// AAD format for person PII
var aad = Encoding.UTF8.GetBytes($"{tenantId}|person_pii|{fieldName}");

// Example usage per field
var nameAad = Encoding.UTF8.GetBytes($"{tenantId}|person_pii|FirstName");
var emailAad = Encoding.UTF8.GetBytes($"{tenantId}|person_pii|Email");
```

Each field gets unique AAD. Swapping `FirstName` ciphertext into `Email` column causes auth tag verification failure on decrypt.

### DEK Wrapping Pattern

DEKs follow envelope encryption: master key wraps individual DEKs.

```csharp
// Master key from configuration (e.g., "PersonEncryption:MasterKey")
// Never in source code — use env vars, vault, or secrets manager

// Wrapped DEK stored in DB: [nonce(12 bytes)][tag(16 bytes)][ciphertext]
// Each person has own DEK, wrapped by master key
public class PersonEncryptionKey
{
    public Guid Id { get; set; }
    public Guid PersonId { get; set; }
    public byte[] WrappedDek { get; set; } = [];  // nonce(12) + tag(16) + encrypted_dek
    public int KeyVersion { get; set; }
    public DateTime CreatedAt { get; set; }
}
```

**Flow:**
1. Master key loaded once at startup (or from vault)
2. Per person, generate random 256-bit DEK
3. Wrap DEK with master key using AES-256-GCM: `AesGcm.Encrypt(masterKey, nonce, plaintextDek, ciphertextDek, tag, aad)`
4. Store wrapped DEK as `nonce + tag + ciphertext` in single `bytea` column
5. On decrypt: extract nonce/tag/ciphertext from `WrappedDek`, unwrap using master key, then use unwrapped DEK to decrypt field data

### Search Column Backfill

Search columns (e.g., `FirstNameSearch`, `LastNameSearch`, `EmailSearch`) always populated regardless of encryption state. Enables backward-compatible queries when encryption toggled on/off:

```csharp
public class Person
{
    public Guid Id { get; set; }

    // Encrypted PII fields
    public byte[]? FirstNameEncrypted { get; set; }
    public byte[]? LastNameEncrypted { get; set; }
    public byte[]? EmailEncrypted { get; set; }

    // Search columns — always populated, even when encryption enabled
    public string? FirstNameSearch { get; set; }
    public string? LastNameSearch { get; set; }
    public string? EmailSearch { get; set; }
}

// Always populate search columns with normalized values
person.FirstNameSearch = input.FirstName?.ToUpperInvariant();
person.LastNameSearch = input.LastName?.ToUpperInvariant();
person.EmailSearch = input.Email?.ToUpperInvariant();
```

**Why:** If encryption off (dev, testing, migration), search columns work. If encryption on, search queries use search columns (indexable, performant). Use `ToUpperInvariant()` (or `ToLowerInvariant()`) for case-insensitive matching.

### Envelope Encryption with Transit Secrets Engine

For production with centralized key management, use secrets engine (HashiCorp Vault, OpenBao) with Transit backend. App never sees KEK; receives wrapped DEKs.

```csharp
// Application-layer encryption contract
public interface IVaultService
{
    Task<byte[]> EncryptAsync(Guid tenantId, Guid credentialId, string plaintext);
    Task<string> DecryptAsync(Guid tenantId, Guid credentialId, byte[] ciphertext);
    Task RotateKeyAsync(Guid tenantId);
}
```

**Flow:**
1. App requests DEK from Transit engine
2. Transit returns DEK wrapped (encrypted) with KEK
3. App caches wrapped DEK in-process only (never Redis/Dragonfly)
4. On decrypt: app sends wrapped DEK + ciphertext to Transit, which unwraps + decrypts
5. Or: app unwraps locally using AES-256-GCM with plaintext DEK (cached in-process)

```csharp
// AES-256-GCM encryption with tenant-bound AAD
public class AesGcmEncryptor
{
    public (byte[] Ciphertext, byte[] Nonce, byte[] Tag) Encrypt(
        byte[] plaintext, byte[] key, Guid tenantId, Guid credentialId)
    {
        var nonce = new byte[AesGcm.NonceByteSizes.MaxSize];
        RandomNumberGenerator.Fill(nonce);

        // AAD binds ciphertext to tenant+credential — prevents transplant attacks
        var aad = Encoding.UTF8.GetBytes($"{tenantId}|{credentialId}|AES-256-GCM");

        var ciphertext = new byte[plaintext.Length];
        var tag = new byte[AesGcm.TagByteSizes.MaxSize];

        using var aes = new AesGcm(key, tag.Length);
        aes.Encrypt(nonce, plaintext, ciphertext, tag, aad);

        return (ciphertext, nonce, tag);
    }

    public byte[] Decrypt(
        byte[] ciphertext, byte[] nonce, byte[] tag, byte[] key,
        Guid tenantId, Guid credentialId)
    {
        var aad = Encoding.UTF8.GetBytes($"{tenantId}|{credentialId}|AES-256-GCM");
        var plaintext = new byte[ciphertext.Length];

        using var aes = new AesGcm(key, tag.Length);
        aes.Decrypt(nonce, ciphertext, tag, plaintext, aad);

        return plaintext;
    }
}
```

**Key rotation:** Call `RotateKeyAsync()` to create new KEK version in Transit, rewrap stored DEK, evict in-process cache. Existing ciphertexts remain readable because DEK rewrapped (not regenerated).

**Persistent storage (EF Core):**
```csharp
public class EncryptionKey
{
    public Guid Id { get; set; }
    // TenantId managed as shadow property by Finbuckle — do NOT declare
    public byte[] WrappedDek { get; set; } = [];
    public int KeyVersion { get; set; }
    public DateTime CreatedAt { get; set; }
}

// Credential with encrypted fields
public class Credential
{
    public Guid Id { get; set; }
    public byte[] Ciphertext { get; set; } = [];
    public byte[] Nonce { get; set; } = [];
    public byte[] Tag { get; set; } = [];
    public byte[] Aad { get; set; } = [];
    public int KeyVersion { get; set; }
}
```

## 6. Key Rotation

**KeyRotationJob:** Scheduled job (Quartz, Hangfire, or background service) querying records encrypted with previous key, re-encrypting with current key.

**Migration service:** For initial plaintext-to-encrypted conversion, build batch migration processing records in chunks. Include round-trip verification: encrypt → decrypt → compare.

```csharp
public async Task MigrateToEncryptionAsync<TEntity>(
    Expression<Func<TEntity, string>> plainField,
    Expression<Func<TEntity, byte[]>> encryptedField,
    int batchSize = 100)
{
    var unencrypted = await _db.Set<TEntity>()
        .Where(/* encryptedField is null */)
        .Take(batchSize)
        .ToListAsync();

    foreach (var entity in unencrypted)
    {
        var plain = plainField.Compile()(entity);
        var encrypted = _encryption.Encrypt(plain, _encryption.CurrentKeyId);
        // Verify round-trip
        var decrypted = _encryption.Decrypt(encrypted, _encryption.CurrentKeyId);
        if (decrypted != plain)
            throw new EncryptionVerificationException(entity.GetType().Name);
        // Update and save
    }
}
```

**Zero-downtime rotation:** Keep current + previous key available. Dual-key fallback ensures records with either key readable during migration window.

**Audit:** Log every re-encryption: entity, record, old key ID, new key ID, timestamp, success/failure.

**IMPORTANT:** Leave marker comment for sensitive data re-encryption if initial migration covers only non-sensitive fields. Sensitive data needs separate, carefully validated migration pass.

## 7. Audit Trail

**EF Core SaveChanges interceptor:** Intercept all DB mutations, create `AuditLog` entries: entity type, operation (INSERT/UPDATE/DELETE), changed field names with old/new values, actor (user ID), timestamp.

**Soft-delete conversion:** Override interceptor to convert DELETE → UPDATE setting `IsDeleted = true`. Prevents accidental hard deletes.

```csharp
public class AuditSaveChangesInterceptor : SaveChangesInterceptor
{
    public override InterceptionResult<int> SavingChanges(DbContextEventData eventData, InterceptionResult<int> result)
    {
        var context = eventData.Context;
        var entries = context.ChangeTracker.Entries()
            .Where(e => e.State is EntityState.Added or EntityState.Modified or EntityState.Deleted);

        foreach (var entry in entries)
        {
            if (entry.State == EntityState.Deleted)
                entry.State = EntityState.Modified; // Convert to soft-delete

            var auditLog = AuditLog.Create(entry, _currentUser.Id);
            context.Add(auditLog);
        }
        return base.SavingChanges(eventData, result);
    }
}
```

**Data subject reconstruction:** Query joining audit logs with sensitive entities to produce complete change timeline across all tables.

**Masking in audit logs:** Don't store encrypted/sensitive values in plaintext. Mask as `[ENCRYPTED]` or store only field name + fact of change.

### Cross-Tenant Audit Entries

With Finbuckle multi-tenancy and shadow `TenantId`, audit entries needing cross-tenant visibility (vault ops, security events, DPO access) must store `TenantId` explicitly. One exception to "never declare TenantId on entities" rule.

```csharp
// This entity intentionally stores TenantId explicitly
// for cross-tenant audit queries (security compliance)
public class CrossTenantAuditEntry
{
    public Guid Id { get; set; }
    public Guid TenantId { get; set; } // Explicit — NOT Finbuckle shadow property
    public Guid UserId { get; set; }
    public string Operation { get; set; } = "";
    public int KeyVersion { get; set; }
    public string IpAddress { get; set; } = "";
    public DateTime Timestamp { get; set; }
}
```

Query all tenants' vault audit entries:
```csharp
var allTenantAudits = await context.CrossTenantAuditEntries
    .Where(e => e.Timestamp >= since)
    .OrderByDescending(e => e.Timestamp)
    .ToListAsync();
```

Without explicit `TenantId`, Finbuckle's query filter restricts to current tenant only, preventing DPO/security from seeing cross-tenant activity.

## 8. Data Retention

**LGPD Arts. 15-16:** Process/store personal data only as long as necessary for stated purpose. After purpose served, delete or anonymize.

**Retention by purpose, not convenience:** Define retention periods per data category + processing purpose, not per entity. E.g., transaction records: 5 years (legal obligation for financial records); marketing preferences: only while consent active.

**Triggers for deletion/anonymization:**
- Processing purpose ended
- Consent revoked with no other legal basis
- Legal retention obligation fulfilled
- Data subject exercised deletion right

**Exceptions:**
- Anonymized data (no longer personal data)
- Data under separate legal obligation (tax, financial, health records)
- Anonymized data for research/statistics

**Implementation:** Entity-level retention policies (purpose, period, expiry action: delete or anonymize). Scheduled cleanup job evaluating policies, processing expired data in batches.

### Data Retention Lifecycle Implementation

Three stages: soft-delete (recycle bin), scheduled hard-delete, audit trail.

```csharp
public interface IRetentionService
{
    Task<PagedResult<SoftDeletedItem>> BrowseRecycleBinAsync(
        Guid tenantId, string entityType, int page, int pageSize);
    Task RestoreAsync(Guid tenantId, Guid entityId, string entityType);
    Task TriggerScheduledCleanupAsync();
}

public class RetentionAuditLog
{
    public Guid Id { get; set; }
    public Guid TenantId { get; set; }
    public string EntityType { get; set; } = "";
    public Guid EntityId { get; set; }
    public DateTime OriginalDeletedAt { get; set; }
    public DateTime HardDeletedAt { get; set; }
    public string RetentionJobRunId { get; set; } = "";
}

public class RetentionJobRun
{
    public Guid Id { get; set; }
    public Guid TenantId { get; set; }
    public DateTime StartedAt { get; set; }
    public DateTime? CompletedAt { get; set; }
    public int EntitiesProcessed { get; set; }
    public int EntitiesDeleted { get; set; }
    public string Status { get; set; } = ""; // Running, Completed, Failed
    public string? ErrorMessage { get; set; }
}
```

Retention service uses reflection across entity types:
```csharp
public async Task HardDeleteExpiredAsync(Guid tenantId, TimeSpan retentionPeriod)
{
    var cutoff = DateTime.UtcNow - retentionPeriod;
    var entityTypes = _db.Model.GetEntityTypes()
        .Where(t => typeof(Entity).IsAssignableFrom(t.ClrType));

    foreach (var entityType in entityTypes)
    {
        var records = await _db.Set(entityType.ClrType)
            .Where(e => EF.Property<bool>(e, "IsDeleted") && EF.Property<DateTime>(e, "DeletedAt") < cutoff)
            .ToListAsync();

        foreach (var record in records)
        {
            _db.Remove(record);
            _retentionAuditLogs.Add(new RetentionAuditLog
            {
                EntityId = (Guid)entityType.FindPrimaryKey()!.Properties[0].PropertyInfo!.GetValue(record)!,
                EntityType = entityType.ClrType.Name,
                OriginalDeletedAt = EF.Property<DateTime>(record, "DeletedAt"),
                HardDeletedAt = DateTime.UtcNow
            });
        }
    }

    await _db.SaveChangesAsync();
}
```

> **Warning:** Schedule with Quartz. Run off-peak. Always write `RetentionAuditLog` before hard-delete for compliance trail.

## 9. International Data Transfer

**Art. 33:** Transfer to other countries only when destination provides adequate protection or controller uses approved safeguards.

**ANPD-approved mechanisms:**
- Adequacy decisions by ANPD
- Standard Contractual Clauses (SCCs)
- Binding Corporate Rules (BCRs)
- Specific contractual clauses approved by ANPD
- International cooperation agreements

**As of 2026:** EU recognized as adequate by ANPD. Verify current adequacy status before relying.

**Practical rules:**
- Transfer notice/consent in Portuguese
- Don't hardcode adequate countries list. ANPD list changes. Check dynamically or review periodically.
- Document which mechanism relied on for each international transfer

## 10. DPO (Encarregado) Obligations

**Publish real contact point.** DPO name and contact must be publicly accessible. Generic "privacy@example.com" with no monitoring insufficient.

**Route DSARs and complaints through DPO.** DPO is bridge between data subjects, controller, and ANPD. All privacy requests funnel through this role.

**Small agents ("agente de tratamento de pequeno porte")** may qualify for some exemptions under Art. 18, but core obligations (consent, data subject rights, breach notification) still apply.

**Independence:** DPO must not have conflicts of interest. Not same person who decides data processing purposes. Relevant in small startups with overlapping roles.

## 11. Privacy Impact Assessment (RIPD)

LGPD doesn't mandate RIPD for all processing. ANPD can require one; mandatory when processing likely results in high risk to civil liberties and fundamental rights.

**When to conduct RIPD:**
- Processing sensitive data (health, biometric, genetic, political, religious)
- Large-scale profiling/automated decision-making
- Systematic monitoring of publicly accessible areas
- Processing vulnerable subjects (children, elderly)
- Combining datasets in unexpected ways
- New processing changing purpose or scope of existing processing

**RIPD structure:**

```csharp
public record PrivacyImpactAssessment(
    string ProcessingActivity,
    string Description,
    string LegalBasis,
    IReadOnlyList<string> DataCategories,
    IReadOnlyList<string> DataSubjects,
    RiskLevel Risk,
    IReadOnlyList<MitigationMeasure> Mitigations,
    DateTime AssessedAt,
    string Assessor
);

public record MitigationMeasure(
    string Description,
    string ImplementationStatus, // Proposed | Implemented | Verified
    string ResponsibleParty
);
```

**Risk scoring matrix:**

| Factor | Low (1) | Medium (2) | High (3) |
|--------|---------|------------|----------|
| Data sensitivity | Non-sensitive | Identifiers | Sensitive (health, biometric) |
| Volume | <1K subjects | 1K-100K | >100K subjects |
| Duration | Temporary (<30 days) | Ongoing with expiry | Indefinite |
| Scope | Internal only | Shared with partners | Public |
| Automation | Manual | Semi-automated | Fully automated decisions |

Multiply factors. Total 1-5: low risk. 6-15: medium. 16-243: high. High-risk assessments need mitigation measures + DPO review.

**Living document:** RIPD not one-time. Review/update when processing changes, new data categories introduced, or after security incident.

## 12. LGPD vs GDPR Quick Reference

| Aspect | LGPD | GDPR |
|--------|------|------|
| Rights response time | 15 days (ANPD guidance) | 1 month (extendable to 3) |
| Breach notification to authority | 3 business days | 72 hours |
| Maximum fine | 2% revenue, capped BRL 50M per infraction | EUR 20M or 4% global turnover |
| International transfer | ANPD adequacy decisions + approved safeguards | Chapter V adequacy, SCCs, BCRs |
| Legal bases | 10 bases (Art. 7) | 6 bases (Art. 6) |
| Sensitive data definition | Broader (includes union membership, genetic) | Special categories (Art. 9) |
| DPO requirement | Mandatory for controllers, optional for processors | Mandatory in most cases |
| Data portability | Yes (Art. 18) | Yes (Art. 20) |

Both laws share same philosophical foundation. If GDPR-compliant, most of way to LGPD compliance. Main differences: tighter timelines (breach notification, response time), different fine structures, ANPD-specific admin requirements.

## 13. Schema-per-Tenant Data Isolation

PostgreSQL schema-per-tenant provides physical data isolation. Each tenant gets own schema, queries routed via `search_path`. Strongest isolation short of separate databases.

**Why schema-per-tenant for LGPD:** Physical isolation makes it trivial to prove one tenant's data cannot leak to another. DSAR, portability, deletion target single schema without row-level filtering. Audit trails inherently scoped.

### SchemaPerTenantStrategy

Centralizes schema naming, sanitization, connection configuration.

```csharp
public static class SchemaPerTenantStrategy
{
    private static readonly Regex ValidIdentifier = new(@"^[a-z_][a-z0-9_]{0,62}$", RegexOptions.Compiled);

    /// <summary>
    /// Produces safe schema name: tenant_{sanitized_identifier}.
    /// Rejects identifiers enabling SQL injection.
    /// </summary>
    public static string SchemaName(string tenantIdentifier)
    {
        if (string.IsNullOrWhiteSpace(tenantIdentifier))
            throw new ArgumentException("Tenant identifier cannot be empty.", nameof(tenantIdentifier));

        var sanitized = tenantIdentifier
            .Trim()
            .ToLowerInvariant()
            .Replace("-", "_")
            .Replace(" ", "_");

        var schema = $"tenant_{sanitized}";

        if (!ValidIdentifier.IsMatch(schema))
            throw new InvalidOperationException(
                $"Sanitized schema name '{schema}' not valid PostgreSQL identifier. " +
                "Tenant identifier contains invalid characters.");

        if (schema.Length > 63)
            throw new InvalidOperationException(
                $"Schema name '{schema}' exceeds PostgreSQL 63-char identifier limit.");

        return schema;
    }

    /// <summary>
    /// Appends search_path to connection string for tenant-scoped queries.
    /// </summary>
    public static string ConnectionStringWithSearchPath(
        string baseConnectionString, string schema)
    {
        var builder = new NpgsqlConnectionStringBuilder(baseConnectionString)
        {
            SearchPath = schema
        };
        return builder.ConnectionString;
    }
}
```

### TenantStoreDbContext

Stores tenant metadata in `public` schema. Only shared table.

```csharp
public class TenantStoreDbContext : DbContext
{
    public DbSet<TenantInfo> Tenants => Set<TenantInfo>();

    public TenantStoreDbContext(DbContextOptions<TenantStoreDbContext> options)
        : base(options) { }

    protected override void OnModelCreating(ModelBuilder modelBuilder)
    {
        modelBuilder.Entity<TenantInfo>(entity =>
        {
            entity.ToTable("tenant_info", "public");
            entity.HasKey(e => e.Id);
            entity.Property(e => e.Identifier).IsRequired().HasMaxLength(100);
            entity.HasIndex(e => e.Identifier).IsUnique();
            entity.Property(e => e.Name).IsRequired().HasMaxLength(200);
            entity.Property(e => e.SchemaName).IsRequired().HasMaxLength(63);
            entity.Property(e => e.CreatedAt).IsRequired();
        });
    }
}

public class TenantInfo
{
    public Guid Id { get; set; }
    public string Identifier { get; set; } = default!;
    public string Name { get; set; } = default!;
    public string SchemaName { get; set; } = default!;
    public DateTime CreatedAt { get; set; }
}
```

### TenantDbContext

Per-tenant context. Receives schema name, sets `search_path` for correct tenant.

```csharp
public class TenantDbContext : DbContext
{
    private readonly string _schema;

    public TenantDbContext(DbContextOptions<TenantDbContext> options, string schema)
        : base(options)
    {
        _schema = !string.IsNullOrEmpty(schema)
            ? schema
            : throw new ArgumentException("Schema must be provided.", nameof(schema));
    }

    protected override void OnConfiguring(DbContextOptionsBuilder optionsBuilder)
    {
        // Npgsql sets search_path from connection string SearchPath parameter.
        // If not set in connection string, apply via raw SQL after connection.
    }

    protected override void OnModelCreating(ModelBuilder modelBuilder)
    {
        // All entities default to tenant's schema
        modelBuilder.HasDefaultSchema(_schema);

        // Register tenant-scoped entities:
        // modelBuilder.Entity<SubjectProfile>(...);
        // modelBuilder.Entity<ConsentRecord>(...);
        // modelBuilder.Entity<AuditLog>(...);
    }

    public async Task SetSearchPathAsync(CancellationToken ct = default)
    {
        await Database.OpenConnectionAsync(ct);
        using var cmd = Database.GetDbConnection().CreateCommand();
        cmd.CommandText = $"SET search_path TO \"{_schema}\", public";
        await cmd.ExecuteNonQueryAsync(ct);
    }
}
```

### MultiTenantMigrationService

Discovers all `tenant_*` schemas from `information_schema` and applies pending migrations to each.

```csharp
public class MultiTenantMigrationService
{
    private readonly IServiceProvider _serviceProvider;
    private readonly string _adminConnectionString;

    public MultiTenantMigrationService(
        IServiceProvider serviceProvider,
        IOptions<DatabaseSettings> settings)
    {
        _serviceProvider = serviceProvider;
        _adminConnectionString = settings.Value.AdminConnectionString;
    }

    /// <summary>
    /// Discovers all tenant schemas from DB, applies pending migrations.
    /// Must run BEFORE app starts serving requests.
    /// </summary>
    public async Task MigrateAllTenantsAsync(CancellationToken ct = default)
    {
        var schemas = await DiscoverTenantSchemasAsync(ct);

        foreach (var schema in schemas)
        {
            await MigrateSchemaAsync(schema, ct);
        }
    }

    private async Task<IReadOnlyList<string>> DiscoverTenantSchemasAsync(CancellationToken ct)
    {
        var schemas = new List<string>();

        await using var connection = new NpgsqlConnection(_adminConnectionString);
        await connection.OpenAsync(ct);

        await using var cmd = connection.CreateCommand();
        cmd.CommandText = @"
            SELECT schema_name
            FROM information_schema.schemata
            WHERE schema_name LIKE 'tenant_%'
              AND schema_name NOT IN ('tenant_info')
            ORDER BY schema_name";

        await using var reader = await cmd.ExecuteReaderAsync(ct);
        while (await reader.ReadAsync(ct))
        {
            schemas.Add(reader.GetString(0));
        }

        return schemas;
    }

    private async Task MigrateSchemaAsync(string schema, CancellationToken ct)
    {
        using var scope = _serviceProvider.CreateScope();
        var options = scope.ServiceProvider.GetRequiredService<DbContextOptions<TenantDbContext>>();

        await using var context = new TenantDbContext(options, schema);
        await context.Database.MigrateAsync(ct);
    }
}
```

### TenantProvisioningService

Creates new tenant: schema in PostgreSQL, runs migrations, registers metadata in catalog.

```csharp
public class TenantProvisioningService
{
    private readonly TenantStoreDbContext _storeDb;
    private readonly IServiceProvider _serviceProvider;
    private readonly string _adminConnectionString;

    public TenantProvisioningService(
        TenantStoreDbContext storeDb,
        IServiceProvider serviceProvider,
        IOptions<DatabaseSettings> settings)
    {
        _storeDb = storeDb;
        _serviceProvider = serviceProvider;
        _adminConnectionString = settings.Value.AdminConnectionString;
    }

    public async Task<TenantInfo> ProvisionAsync(
        string identifier, string name, CancellationToken ct = default)
    {
        var schema = SchemaPerTenantStrategy.SchemaName(identifier);

        await CreateSchemaIfNotExistsAsync(schema, ct);
        await MigrateSchemaAsync(schema, ct);

        var tenant = new TenantInfo
        {
            Id = Guid.NewGuid(),
            Identifier = identifier,
            Name = name,
            SchemaName = schema,
            CreatedAt = DateTime.UtcNow
        };

        _storeDb.Tenants.Add(tenant);
        await _storeDb.SaveChangesAsync(ct);

        return tenant;
    }

    private async Task CreateSchemaIfNotExistsAsync(string schema, CancellationToken ct)
    {
        await using var connection = new NpgsqlConnection(_adminConnectionString);
        await connection.OpenAsync(ct);

        await using var cmd = connection.CreateCommand();
        // Schema name validated by SchemaPerTenantStrategy — safe to interpolate.
        cmd.CommandText = $"""
            CREATE SCHEMA IF NOT EXISTS "{schema}";
            GRANT USAGE, CREATE ON SCHEMA "{schema}" TO CURRENT_USER;
            """;
        await cmd.ExecuteNonQueryAsync(ct);
    }

    private async Task MigrateSchemaAsync(string schema, CancellationToken ct)
    {
        using var scope = _serviceProvider.CreateScope();
        var options = scope.ServiceProvider.GetRequiredService<DbContextOptions<TenantDbContext>>();

        await using var context = new TenantDbContext(options, schema);
        await context.Database.MigrateAsync(ct);
    }
}
```

**Key LGPD compliance points:**

- Each tenant's personal data in physically separate schema. No row-level security needed for cross-tenant prevention.
- `search_path` routes all EF Core queries to correct schema transparently.
- Metadata (tenant catalog) always in `public`. Tenant business data never.
- Schema name sanitization via `SchemaPerTenantStrategy` prevents SQL injection.
- DSAR handlers receive tenant schema, query only that scope. Deletion/anonymization of one tenant cannot affect another.
- When data subject exists in multiple tenants, each DSAR handled independently per tenant.

## 14. Anti-Patterns

**NEVER hard-delete regulated data.** Soft-delete only. Hard-deleting financial/health/legally-retained data creates compliance exposure.

**NEVER store OAuth tokens, API keys, calendar credentials in plaintext.** Use encryption service for all credentials at rest.

**NEVER log sensitive tokens, credentials, personal data.** Audit logs capture metadata (what changed, who, when) not actual values.

**NEVER transmit personal data off-device without encryption.** E.g., do speech-to-text locally; only text transcript leaves device.

**NEVER bundle consent across purposes.** Each purpose needs own consent record. Single "I accept everything" checkbox violates LGPD Art. 8.

**NEVER ignore breach notification deadlines.** Three business days to ANPD is tight. Test notification workflow before needed.

**NEVER hardcode lists of countries approved for data transfer.** ANPD adequacy list changes. Check dynamically or review quarterly.

**NEVER store tenant data in public schema with schema-per-tenant.** `public` schema for catalog metadata (`tenant_info`) only. Business/personal/consent/audit data in tenant-scoped schemas.

**NEVER run migrations without discovering existing schemas.** Query `information_schema.schemata` for `tenant_*` schemas first. Blindly assuming fixed list causes schema drift.

**NEVER skip schema name sanitization.** Raw identifiers in `CREATE SCHEMA` or `SET search_path` are SQL injection vectors. Route through `SchemaPerTenantStrategy.SchemaName`.

**NEVER cache wrapped DEKs in external caches (Redis, Dragonfly, Memcached).** DEKs cached in-process only. External caching exposes key material to network attacks.

**NEVER store audit log entries for vault/encryption ops without explicit TenantId.** Finbuckle shadow property prevents DPO from cross-tenant queries. Vault/encryption audit logs are exception to shadow-property rule.

## 15. Implementation Caveats

### EF Core Expression Tree Limitations

EF Core expression tree lambdas CANNOT contain:
- **Dictionary initializers** (CS8074) — `new Dictionary<string, object> { ["key"] = value }` untranslatable
- **Null propagating operators** (CS8072) — `entity?.Property` unusable in expressions

**Fix:** Materialize query with anonymous type projection, convert in-memory:

```csharp
// WRONG — EF Core cannot translate
var result = await _db.Set<Person>()
    .Select(p => new Dictionary<string, object?>
    {
        ["Name"] = p.Name,
        ["Email"] = p.Email
    })
    .ToListAsync();

// CORRECT — project to anonymous type, convert in-memory
var result = await _db.Set<Person>()
    .Select(p => new { p.Name, p.Email })
    .ToListAsync();  // Materialize

var dict = result.Select(r => new Dictionary<string, object?>
{
    ["Name"] = r.Name,
    ["Email"] = r.Email
}).ToList();
```

### CA1869: Cache JsonSerializerOptions

Creating new `JsonSerializerOptions` per serialization triggers CA1869 warning + wastes allocations. Cache as static readonly:

```csharp
private static readonly JsonSerializerOptions _jsonOptions = new()
{
    WriteIndented = false
};
```

## Related Skills

- **dotnet-clean-arch** — Encryption at rest and audit trail patterns
- **postgresql-efcore** — pgcrypto setup, soft-delete interceptors, encrypted bytea columns, schema-per-tenant migrations
- **quartz-scheduling** — Background jobs for key rotation, data retention cleanup, consent expiry, tenant provisioning
- **keycloak-oidc** — Auth patterns for audit trail actor tracking, multi-tenant OIDC client isolation