param(
  [string]$Overlay = "",
  [string]$Compact = "",
  [string]$Out = "build\preview\capture.png",
  [int]$WaitMs = 4500,
  [ValidateSet("screen", "print")]
  [string]$Method = "screen"
)

$ErrorActionPreference = "Stop"

$exe = "build\windows\x64\runner\Debug\proxy_open_hub.exe"
if (-not (Test-Path "build\windows\x64\runner\Debug\data\app.so") -and
    (Test-Path "build\windows\x64\runner\Release\proxy_open_hub.exe")) {
  $exe = "build\windows\x64\runner\Release\proxy_open_hub.exe"
}
if (-not (Test-Path $exe)) { throw "Build not found: $exe" }

New-Item -ItemType Directory -Force -Path (Split-Path $Out) | Out-Null

Get-Process proxy_open_hub -ErrorAction SilentlyContinue | ForEach-Object {
  try {
    $_.Kill()
    $_.WaitForExit(2000)
  } catch {
    # Best-effort cleanup for deterministic screenshots.
  }
}

$env:POH_DEBUG_OVERLAY = $Overlay
$env:POH_DEBUG_COMPACT = $Compact

Add-Type -AssemblyName System.Drawing
Add-Type -ReferencedAssemblies "System.Drawing" @"
using System;
using System.Drawing;
using System.Runtime.InteropServices;
public class Win {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int n);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr insertAfter, int x, int y, int cx, int cy, uint flags);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hwnd, IntPtr hdcBlt, uint nFlags);

  public static readonly IntPtr HWND_TOPMOST = new IntPtr(-1);
  public const uint SWP_NOSIZE = 0x0001;
  public const uint SWP_SHOWWINDOW = 0x0040;
  public const uint PW_RENDERFULLCONTENT = 0x00000002;

  public static void CaptureWithPrintWindow(IntPtr hwnd, string path, int width, int height) {
    using (Bitmap bitmap = new Bitmap(width, height)) {
      using (Graphics graphics = Graphics.FromImage(bitmap)) {
        IntPtr hdc = graphics.GetHdc();
        try {
          PrintWindow(hwnd, hdc, PW_RENDERFULLCONTENT);
        } finally {
          graphics.ReleaseHdc(hdc);
        }
      }
      bitmap.Save(path, System.Drawing.Imaging.ImageFormat.Png);
    }
  }
}
"@

$p = Start-Process -FilePath $exe -PassThru
try {
  $handle = [IntPtr]::Zero
  for ($i = 0; $i -lt 60; $i++) {
    Start-Sleep -Milliseconds 250
    $p.Refresh()
    if ($p.MainWindowHandle -ne [IntPtr]::Zero) { $handle = $p.MainWindowHandle; break }
  }
  if ($handle -eq [IntPtr]::Zero) { throw "No main window handle appeared" }

  # Give Flutter time to settle layout + run the entrance animation.
  Start-Sleep -Milliseconds $WaitMs
  [Win]::ShowWindow($handle, 9) | Out-Null   # SW_RESTORE
  [Win]::SetWindowPos(
    $handle,
    [Win]::HWND_TOPMOST,
    24,
    24,
    0,
    0,
    [Win]::SWP_NOSIZE -bor [Win]::SWP_SHOWWINDOW
  ) | Out-Null
  [Win]::SetForegroundWindow($handle) | Out-Null
  Start-Sleep -Milliseconds 900

  $r = New-Object Win+RECT
  [Win]::GetWindowRect($handle, [ref]$r) | Out-Null
  $w = $r.R - $r.L
  $h = $r.B - $r.T
  if ($w -le 0 -or $h -le 0) { throw "Bad window rect $w x $h" }

  $targetDir = (Resolve-Path -LiteralPath (Split-Path $Out)).Path
  $target = $targetDir + "\" + (Split-Path $Out -Leaf)

  $bmp = New-Object System.Drawing.Bitmap $w, $h
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  try {
    if ($Method -eq "print") {
      $g.Dispose()
      $bmp.Dispose()
      [Win]::CaptureWithPrintWindow($handle, $target, $w, $h)
    } else {
      $g.CopyFromScreen($r.L, $r.T, 0, 0, (New-Object System.Drawing.Size $w, $h))
      $bmp.Save($target, [System.Drawing.Imaging.ImageFormat]::Png)
      $g.Dispose()
      $bmp.Dispose()
    }
  } finally {
    if ($g -ne $null) { $g.Dispose() }
    if ($bmp -ne $null) { $bmp.Dispose() }
  }
  Write-Output "Saved $Out ($w x $h, $Method)"
}
finally {
  if (-not $p.HasExited) { $p.Kill() }
}
