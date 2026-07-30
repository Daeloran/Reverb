<#
    Envoie des commandes de controle a l'ecran du Kraken (1e71:300c).

    Rejoue les commandes observees dans la capture, en faisant varier leurs
    parametres pour identifier ce qu'ils pilotent :

        30 02 01 <lum> 00 00 00 00 1e     luminosite supposee
        38 01 <mode>                      mode d affichage

    Sans accents (UTF-8 sans BOM + PowerShell 5.1).
#>
[CmdletBinding()]
param(
    [switch] $ListOnly,
    [int] $Luminosite = -1,
    [int] $Mode = -1
)

$ErrorActionPreference = 'Stop'

Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;

public class KrakenHid
{
    const int DIGCF_PRESENT = 0x02;
    const int DIGCF_DEVICEINTERFACE = 0x10;
    const uint GENERIC_WRITE = 0x40000000;
    const uint FILE_SHARE_READ = 1;
    const uint FILE_SHARE_WRITE = 2;
    const uint OPEN_EXISTING = 3;

    [StructLayout(LayoutKind.Sequential)]
    struct SP_DEVICE_INTERFACE_DATA
    { public int cbSize; public Guid InterfaceClassGuid; public int Flags; public IntPtr Reserved; }

    [StructLayout(LayoutKind.Sequential)]
    struct HIDD_ATTRIBUTES
    { public int Size; public ushort VendorID; public ushort ProductID; public ushort VersionNumber; }

    [DllImport("hid.dll")] static extern void HidD_GetHidGuid(out Guid g);
    [DllImport("hid.dll")] static extern bool HidD_GetAttributes(IntPtr h, ref HIDD_ATTRIBUTES a);
    [DllImport("setupapi.dll", CharSet = CharSet.Unicode)]
    static extern IntPtr SetupDiGetClassDevs(ref Guid g, IntPtr e, IntPtr w, int f);
    [DllImport("setupapi.dll")]
    static extern bool SetupDiEnumDeviceInterfaces(IntPtr s, IntPtr d, ref Guid g, int i, ref SP_DEVICE_INTERFACE_DATA dia);
    [DllImport("setupapi.dll", CharSet = CharSet.Unicode)]
    static extern bool SetupDiGetDeviceInterfaceDetail(IntPtr s, ref SP_DEVICE_INTERFACE_DATA dia, IntPtr det, int size, out int req, IntPtr di);
    [DllImport("setupapi.dll")] static extern bool SetupDiDestroyDeviceInfoList(IntPtr s);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    static extern IntPtr CreateFile(string n, uint a, uint sh, IntPtr sec, uint d, uint f, IntPtr t);
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool WriteFile(IntPtr h, byte[] b, int l, out int w, IntPtr o);
    [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);

    public static List<string> Find(ushort vid, ushort pid)
    {
        var res = new List<string>();
        Guid g; HidD_GetHidGuid(out g);
        IntPtr set = SetupDiGetClassDevs(ref g, IntPtr.Zero, IntPtr.Zero, DIGCF_PRESENT | DIGCF_DEVICEINTERFACE);
        if (set == IntPtr.Zero || set.ToInt64() == -1) return res;
        try
        {
            var dia = new SP_DEVICE_INTERFACE_DATA();
            dia.cbSize = Marshal.SizeOf(typeof(SP_DEVICE_INTERFACE_DATA));
            for (int i = 0; SetupDiEnumDeviceInterfaces(set, IntPtr.Zero, ref g, i, ref dia); i++)
            {
                int need; SetupDiGetDeviceInterfaceDetail(set, ref dia, IntPtr.Zero, 0, out need, IntPtr.Zero);
                if (need <= 0) continue;
                IntPtr det = Marshal.AllocHGlobal(need);
                try
                {
                    Marshal.WriteInt32(det, (IntPtr.Size == 8) ? 8 : 6);
                    int dummy;
                    if (!SetupDiGetDeviceInterfaceDetail(set, ref dia, det, need, out dummy, IntPtr.Zero)) continue;
                    string path = Marshal.PtrToStringUni(new IntPtr(det.ToInt64() + 4));
                    if (path == null) continue;
                    IntPtr h = CreateFile(path, 0, FILE_SHARE_READ | FILE_SHARE_WRITE, IntPtr.Zero, OPEN_EXISTING, 0, IntPtr.Zero);
                    if (h.ToInt64() == -1) continue;
                    try
                    {
                        var at = new HIDD_ATTRIBUTES(); at.Size = Marshal.SizeOf(typeof(HIDD_ATTRIBUTES));
                        if (!HidD_GetAttributes(h, ref at)) continue;
                        if (at.VendorID == vid && at.ProductID == pid) res.Add(path);
                    }
                    finally { CloseHandle(h); }
                }
                finally { Marshal.FreeHGlobal(det); }
            }
        }
        finally { SetupDiDestroyDeviceInfoList(set); }
        return res;
    }

    public static string Write(string path, byte[] payload)
    {
        IntPtr h = CreateFile(path, GENERIC_WRITE, FILE_SHARE_READ | FILE_SHARE_WRITE,
                              IntPtr.Zero, OPEN_EXISTING, 0, IntPtr.Zero);
        if (h.ToInt64() == -1) return "CreateFile erreur " + Marshal.GetLastWin32Error();
        try
        {
            int w;
            if (WriteFile(h, payload, payload.Length, out w, IntPtr.Zero)) return null;
            int e1 = Marshal.GetLastWin32Error();
            byte[] alt = new byte[payload.Length + 1];
            Array.Copy(payload, 0, alt, 1, payload.Length);
            if (WriteFile(h, alt, alt.Length, out w, IntPtr.Zero)) return null;
            return "WriteFile erreurs " + e1 + " et " + Marshal.GetLastWin32Error();
        }
        finally { CloseHandle(h); }
    }
}
'@

$paths = [KrakenHid]::Find(0x1E71, 0x300C)
Write-Host ""
Write-Host "Interfaces HID du Kraken (1e71:300c) :" -ForegroundColor Cyan
$paths | ForEach-Object { "  $_" }
if (-not $paths) { throw "Kraken introuvable." }
Write-Host ""
if ($ListOnly) { return }

$path = $paths[0]

function Envoyer {
    param([byte[]] $Octets, [string] $Quoi)
    $buf = New-Object byte[] 64
    for ($i = 0; $i -lt $Octets.Count; $i++) { $buf[$i] = $Octets[$i] }
    $hex = ($Octets | ForEach-Object { '{0:x2}' -f $_ }) -join ' '
    $err = [KrakenHid]::Write($path, $buf)
    if ($err) { Write-Host ("  ECHEC  {0,-22} [{1}]  {2}" -f $Quoi, $hex, $err) -ForegroundColor Red }
    else      { Write-Host ("  ok     {0,-22} [{1}]" -f $Quoi, $hex) -ForegroundColor Green }
}

if ($Luminosite -ge 0) {
    Envoyer @(0x30,0x02,0x01,[byte]$Luminosite,0x00,0x00,0x00,0x00,0x1e) ("luminosite $Luminosite")
}
if ($Mode -ge 0) {
    Envoyer @(0x38,0x01,[byte]$Mode) ("mode d affichage $Mode")
}
