<#
    Capture l'image d'une fenetre donnee, designee par un fragment de son titre.
    Ne capture que cette fenetre, pas le reste de l'ecran.

    Sans accents (UTF-8 sans BOM + PowerShell 5.1).
#>
param(
    [string] $TitreContient,
    [string] $ProcessName,
    [Parameter(Mandatory = $true)][string] $Out
)

$ErrorActionPreference = 'Stop'

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public class Win {
    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
    // PrintWindow fait dessiner la fenetre dans un contexte a nous : le resultat
    // est correct meme si une autre fenetre la recouvre.
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
}
'@

# Le nom de processus est plus fiable que le titre : un navigateur ouvert sur la
# documentation d'une application porte le nom de celle-ci dans son titre.
if ($ProcessName) {
    $p = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue |
         Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
    if (-not $p) { throw "Aucune fenetre pour le processus '$ProcessName'." }
} else {
    $p = Get-Process | Where-Object { $_.MainWindowTitle -like "*$TitreContient*" } | Select-Object -First 1
    if (-not $p) { throw "Aucune fenetre dont le titre contient '$TitreContient'." }
}

$h = $p.MainWindowHandle
if ([Win]::IsIconic($h)) { [Win]::ShowWindow($h, 9) | Out-Null }   # SW_RESTORE
[Win]::SetForegroundWindow($h) | Out-Null
Start-Sleep -Milliseconds 800

$r = New-Object Win+RECT
if (-not [Win]::GetWindowRect($h, [ref] $r)) { throw "GetWindowRect a echoue." }
$w = $r.Right - $r.Left
$hh = $r.Bottom - $r.Top
if ($w -le 0 -or $hh -le 0) { throw "Dimensions de fenetre invalides." }

Add-Type -AssemblyName System.Drawing
$bmp = New-Object System.Drawing.Bitmap($w, $hh)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$hdc = $g.GetHdc()
# flag 2 = PW_RENDERFULLCONTENT, necessaire pour les fenetres composees
$okPrint = [Win]::PrintWindow($h, $hdc, 2)
$g.ReleaseHdc($hdc)
if (-not $okPrint) {
    # repli : copie depuis l'ecran, correcte seulement si la fenetre est visible
    $g.CopyFromScreen($r.Left, $r.Top, 0, 0, (New-Object System.Drawing.Size($w, $hh)))
}
$g.Dispose()
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()

"fenetre  : $($p.MainWindowTitle)"
"taille   : $w x $hh"
"image    : $Out"
