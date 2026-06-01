namespace TrustTunnel.Core.Validation;

public enum ValidationSeverity
{
    Error,
    Warning
}

public sealed record ValidationIssue(
    string Code,
    string Message,
    ValidationSeverity Severity = ValidationSeverity.Error);

public sealed class ValidationReport
{
    private readonly List<ValidationIssue> _issues = new();

    public IReadOnlyList<ValidationIssue> Issues => _issues;
    public bool IsValid => _issues.All(issue => issue.Severity != ValidationSeverity.Error);

    public void Error(string code, string message) => _issues.Add(new ValidationIssue(code, message));
    public void Warning(string code, string message) => _issues.Add(new ValidationIssue(code, message, ValidationSeverity.Warning));
}
