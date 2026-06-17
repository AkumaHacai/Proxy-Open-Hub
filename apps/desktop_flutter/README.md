# Proxy Open Hub Flutter Shell

This is the source-first Flutter desktop shell for the Rust + Flutter migration.

The repository currently stores only the portable Flutter source files. After the
Flutter SDK is installed, generate platform runner files in this folder:

```powershell
flutter create --platforms=windows .
flutter run -d windows
```

The visual model follows the local references in:

- `C:\Users\mirot\Documents\TT gui\newgui`
- `C:\Users\mirot\Documents\TT gui\newaddgui`

Rust remains responsible for core registry, downloads, verification, config
materialization, process lifecycle, metrics, logs, and diagnostics. Flutter owns
only presentation and user workflow.
