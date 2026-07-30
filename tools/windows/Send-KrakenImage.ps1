<#
    Envoie une image sur l'ecran du Kraken Elite (1e71:300c).

    Deux canaux sont necessaires :
      - HID (interface MI_01) pour les commandes 36 01 / 36 02
      - WinUSB (interface MI_00, pipe bulk 0x02) pour les donnees

    Sequence relevee dans la capture :
        HID   36 01 00 01 09          annonce
        BULK  en-tete de 20 octets
        BULK  1 228 800 octets        640x640, BGR888
        HID   36 02                   validation

    L'image par defaut est une mire a quatre quadrants : elle valide en une
    fois l'envoi, la geometrie et l'ordre des composantes.

    Sans accents (UTF-8 sans BOM + PowerShell 5.1).
#>
[CmdletBinding()]
param(
    [int] $Repetitions = 1,
    [int] $DelaiMs = 900,
    [ValidateSet('quadrants','arcenciel')][string] $Motif = 'quadrants',
    # Mode d affichage a forcer avant l envoi (38 01 <n>). -1 pour ne rien changer.
    [int] $Mode = -1
)

$ErrorActionPreference = 'Stop'

Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;

public class KrakenUsb
{
    const int DIGCF_PRESENT = 0x02;
    const int DIGCF_DEVICEINTERFACE = 0x10;
    const uint GENERIC_READ  = 0x80000000;
    const uint GENERIC_WRITE = 0x40000000;
    const uint FILE_SHARE_READ = 1, FILE_SHARE_WRITE = 2;
    const uint OPEN_EXISTING = 3;
    const uint FILE_FLAG_OVERLAPPED = 0x40000000;

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

    [DllImport("winusb.dll", SetLastError = true)]
    static extern bool WinUsb_Initialize(IntPtr dev, out IntPtr itf);
    [DllImport("winusb.dll", SetLastError = true)]
    static extern bool WinUsb_WritePipe(IntPtr itf, byte pipe, byte[] buf, uint len, out uint transferred, IntPtr ov);
    [DllImport("winusb.dll", SetLastError = true)]
    static extern bool WinUsb_Free(IntPtr itf);

    // ---- chemins de peripheriques -------------------------------------

    static List<string> Interfaces(Guid g)
    {
        var res = new List<string>();
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
                    if (SetupDiGetDeviceInterfaceDetail(set, ref dia, det, need, out dummy, IntPtr.Zero))
                    {
                        string p = Marshal.PtrToStringUni(new IntPtr(det.ToInt64() + 4));
                        if (p != null) res.Add(p);
                    }
                }
                finally { Marshal.FreeHGlobal(det); }
            }
        }
        finally { SetupDiDestroyDeviceInfoList(set); }
        return res;
    }

    public static string CheminHid(ushort vid, ushort pid)
    {
        Guid g; HidD_GetHidGuid(out g);
        foreach (var p in Interfaces(g))
        {
            IntPtr h = CreateFile(p, 0, FILE_SHARE_READ | FILE_SHARE_WRITE, IntPtr.Zero, OPEN_EXISTING, 0, IntPtr.Zero);
            if (h.ToInt64() == -1) continue;
            try
            {
                var a = new HIDD_ATTRIBUTES(); a.Size = Marshal.SizeOf(typeof(HIDD_ATTRIBUTES));
                if (HidD_GetAttributes(h, ref a) && a.VendorID == vid && a.ProductID == pid) return p;
            }
            finally { CloseHandle(h); }
        }
        return null;
    }

    public static string CheminWinUsb(string guid)
    {
        Guid g = new Guid(guid);
        var l = Interfaces(g);
        return l.Count > 0 ? l[0] : null;
    }

    // ---- ecritures -----------------------------------------------------

    public static string EcrireHid(string path, byte[] payload)
    {
        IntPtr h = CreateFile(path, GENERIC_WRITE, FILE_SHARE_READ | FILE_SHARE_WRITE,
                              IntPtr.Zero, OPEN_EXISTING, 0, IntPtr.Zero);
        if (h.ToInt64() == -1) return "CreateFile HID erreur " + Marshal.GetLastWin32Error();
        try
        {
            int w;
            if (WriteFile(h, payload, payload.Length, out w, IntPtr.Zero)) return null;
            return "WriteFile HID erreur " + Marshal.GetLastWin32Error();
        }
        finally { CloseHandle(h); }
    }

    public static string EcrireBulk(string path, byte[] entete, byte[] image)
    {
        IntPtr dev = CreateFile(path, GENERIC_READ | GENERIC_WRITE,
                                FILE_SHARE_READ | FILE_SHARE_WRITE, IntPtr.Zero,
                                OPEN_EXISTING, FILE_FLAG_OVERLAPPED, IntPtr.Zero);
        if (dev.ToInt64() == -1) return "CreateFile WinUSB erreur " + Marshal.GetLastWin32Error();
        IntPtr itf;
        if (!WinUsb_Initialize(dev, out itf))
        { int e = Marshal.GetLastWin32Error(); CloseHandle(dev); return "WinUsb_Initialize erreur " + e; }
        try
        {
            uint sent;
            if (!WinUsb_WritePipe(itf, 0x02, entete, (uint)entete.Length, out sent, IntPtr.Zero))
                return "ecriture de l en-tete : erreur " + Marshal.GetLastWin32Error();
            if (sent != entete.Length) return "en-tete tronque : " + sent + " sur " + entete.Length;

            if (!WinUsb_WritePipe(itf, 0x02, image, (uint)image.Length, out sent, IntPtr.Zero))
                return "ecriture de l image : erreur " + Marshal.GetLastWin32Error();
            if (sent != image.Length) return "image tronquee : " + sent + " sur " + image.Length;
            return null;
        }
        finally { WinUsb_Free(itf); CloseHandle(dev); }
    }

    // ---- generation d image (BGR888, 640x640) ---------------------------

    public static byte[] Quadrants(int w, int h)
    {
        byte[] d = new byte[w * h * 3];
        int cx = w / 2, cy = h / 2;
        for (int y = 0; y < h; y++)
            for (int x = 0; x < w; x++)
            {
                byte r, g, b;
                if (Math.Abs(x - cx) < 5 || Math.Abs(y - cy) < 5) { r = 0; g = 0; b = 0; }
                else if (x < cx && y < cy) { r = 255; g = 0; b = 0; }     // haut gauche : ROUGE
                else if (x >= cx && y < cy) { r = 0; g = 255; b = 0; }    // haut droite : VERT
                else if (x < cx) { r = 0; g = 0; b = 255; }               // bas gauche  : BLEU
                else { r = 255; g = 255; b = 255; }                       // bas droite  : BLANC
                int o = (y * w + x) * 3;
                d[o] = b; d[o + 1] = g; d[o + 2] = r;                     // ordre BGR
            }
        return d;
    }

    public static byte[] ArcEnCiel(int w, int h)
    {
        byte[] d = new byte[w * h * 3];
        double cx = w / 2.0, cy = h / 2.0;
        for (int y = 0; y < h; y++)
            for (int x = 0; x < w; x++)
            {
                double ang = Math.Atan2(y - cy, x - cx);
                double hue = (ang + Math.PI) / (2 * Math.PI) * 6.0;
                double rad = Math.Min(1.0, Math.Sqrt((x-cx)*(x-cx) + (y-cy)*(y-cy)) / (w / 2.0));
                int i = (int)hue; double f = hue - i;
                double v = 1.0, s = rad;
                double p = v * (1 - s), q = v * (1 - s * f), t = v * (1 - s * (1 - f));
                double rr, gg, bb;
                switch (i % 6) {
                    case 0: rr=v; gg=t; bb=p; break;
                    case 1: rr=q; gg=v; bb=p; break;
                    case 2: rr=p; gg=v; bb=t; break;
                    case 3: rr=p; gg=q; bb=v; break;
                    case 4: rr=t; gg=p; bb=v; break;
                    default: rr=v; gg=p; bb=q; break;
                }
                int o = (y * w + x) * 3;
                d[o] = (byte)(bb*255); d[o+1] = (byte)(gg*255); d[o+2] = (byte)(rr*255);
            }
        return d;
    }
}
'@

$GUID_BULK = '{300c300b-7EE7-1125-0724-101503010819}'

$hid  = [KrakenUsb]::CheminHid(0x1E71, 0x300C)
$bulk = [KrakenUsb]::CheminWinUsb($GUID_BULK)

Write-Host ""
Write-Host "HID    : $hid"
Write-Host "WinUSB : $bulk"
Write-Host ""
if (-not $hid)  { throw "Interface HID du Kraken introuvable." }
if (-not $bulk) { throw "Interface WinUSB du Kraken introuvable." }

if ($Motif -eq 'quadrants') { $image = [KrakenUsb]::Quadrants(640, 640) }
else                        { $image = [KrakenUsb]::ArcEnCiel(640, 640) }
Write-Host ("image generee : {0:N0} octets ({1})" -f $image.Length, $Motif)

# en-tete releve dans la capture : signature + longueur en little-endian
$entete = [byte[]] @(0x12,0xfa,0x01,0xe8, 0xab,0xcd,0xef,0x98,0x76,0x54,0x32,0x10,
                     0x09,0x00,0x00,0x00, 0x00,0xc0,0x12,0x00)

function Trame64 { param([byte[]] $o)
    $b = New-Object byte[] 64
    for ($i = 0; $i -lt $o.Count; $i++) { $b[$i] = $o[$i] }
    return $b }

if ($Mode -ge 0) {
    $e = [KrakenUsb]::EcrireHid($hid, (Trame64 @(0x38,0x01,[byte]$Mode)))
    if ($e) { Write-Host "  38 01 $Mode : $e" -ForegroundColor Red }
    else    { Write-Host "  mode d affichage force a $Mode" -ForegroundColor Cyan }
    Start-Sleep -Milliseconds 300
}

for ($n = 1; $n -le $Repetitions; $n++) {
    $e = [KrakenUsb]::EcrireHid($hid, (Trame64 @(0x36,0x01,0x00,0x01,0x09)))
    if ($e) { Write-Host "  36 01 : $e" -ForegroundColor Red; break }

    $e = [KrakenUsb]::EcrireBulk($bulk, $entete, $image)
    if ($e) { Write-Host "  bulk  : $e" -ForegroundColor Red; break }

    $e = [KrakenUsb]::EcrireHid($hid, (Trame64 @(0x36,0x02)))
    if ($e) { Write-Host "  36 02 : $e" -ForegroundColor Red; break }

    Write-Host ("  envoi {0}/{1} ok a {2}" -f $n, $Repetitions, (Get-Date).ToString('HH:mm:ss.fff')) -ForegroundColor Green
    if ($n -lt $Repetitions) { Start-Sleep -Milliseconds $DelaiMs }
}
Write-Host ""
