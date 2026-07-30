<#
    Pilotage direct des controleurs RGB NZXT, sans CAM.

    Implemente la specification etablie dans SPEC-PROTOCOLE-NZXT.md :
      - commande 0x2a 0x04 : couleur fixe sur un canal
      - commande 0x22 0x10 : pilotage LED par LED

    Ecrit en HID brut via SetupAPI + WriteFile (endpoint interrupt OUT),
    exactement comme le fait CAM. Aucune dependance a installer.

    Sans accents (UTF-8 sans BOM + PowerShell 5.1).

    Usage :
        .\Set-NzxtColor.ps1 -ListOnly
        .\Set-NzxtColor.ps1 -R 0 -G 255 -B 0
        .\Set-NzxtColor.ps1 -PerLed
#>
[CmdletBinding()]
param(
    [switch] $ListOnly,
    [int] $R = 0,
    [int] $G = 255,
    [int] $B = 0,
    [switch] $PerLed,
    [double] $Luminosite = 1.0,
    # N'envoie que la trame 0x22 0x10, sans les commandes 0x22 0x11 et 0x22 0xa0.
    # Sert a determiner si ces deux dernieres sont reellement necessaires.
    [switch] $SansValidation
)

$ErrorActionPreference = 'Stop'

Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;

public class NzxtHid
{
    const int DIGCF_PRESENT = 0x02;
    const int DIGCF_DEVICEINTERFACE = 0x10;
    const uint GENERIC_READ  = 0x80000000;
    const uint GENERIC_WRITE = 0x40000000;
    const uint FILE_SHARE_READ  = 1;
    const uint FILE_SHARE_WRITE = 2;
    const uint OPEN_EXISTING = 3;

    [StructLayout(LayoutKind.Sequential)]
    struct SP_DEVICE_INTERFACE_DATA
    {
        public int cbSize;
        public Guid InterfaceClassGuid;
        public int Flags;
        public IntPtr Reserved;
    }

    [StructLayout(LayoutKind.Sequential)]
    struct HIDD_ATTRIBUTES
    {
        public int Size;
        public ushort VendorID;
        public ushort ProductID;
        public ushort VersionNumber;
    }

    [StructLayout(LayoutKind.Sequential)]
    struct HIDP_CAPS
    {
        public ushort Usage;
        public ushort UsagePage;
        public ushort InputReportByteLength;
        public ushort OutputReportByteLength;
        public ushort FeatureReportByteLength;
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 17)]
        public ushort[] Reserved;
        public ushort NumberLinkCollectionNodes;
        public ushort NumberInputButtonCaps;
        public ushort NumberInputValueCaps;
        public ushort NumberInputDataIndices;
        public ushort NumberOutputButtonCaps;
        public ushort NumberOutputValueCaps;
        public ushort NumberOutputDataIndices;
        public ushort NumberFeatureButtonCaps;
        public ushort NumberFeatureValueCaps;
        public ushort NumberFeatureDataIndices;
    }

    [DllImport("hid.dll")] static extern void HidD_GetHidGuid(out Guid g);
    [DllImport("hid.dll")] static extern bool HidD_GetAttributes(IntPtr h, ref HIDD_ATTRIBUTES a);
    [DllImport("hid.dll")] static extern bool HidD_GetPreparsedData(IntPtr h, out IntPtr pp);
    [DllImport("hid.dll")] static extern bool HidD_FreePreparsedData(IntPtr pp);
    [DllImport("hid.dll")] static extern int  HidP_GetCaps(IntPtr pp, out HIDP_CAPS caps);

    [DllImport("setupapi.dll", CharSet = CharSet.Unicode)]
    static extern IntPtr SetupDiGetClassDevs(ref Guid g, IntPtr enumerator, IntPtr hwnd, int flags);
    [DllImport("setupapi.dll")]
    static extern bool SetupDiEnumDeviceInterfaces(IntPtr set, IntPtr devInfo, ref Guid g, int idx, ref SP_DEVICE_INTERFACE_DATA dia);
    [DllImport("setupapi.dll", CharSet = CharSet.Unicode)]
    static extern bool SetupDiGetDeviceInterfaceDetail(IntPtr set, ref SP_DEVICE_INTERFACE_DATA dia, IntPtr detail, int size, out int required, IntPtr devInfo);
    [DllImport("setupapi.dll")]
    static extern bool SetupDiDestroyDeviceInfoList(IntPtr set);

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    static extern IntPtr CreateFile(string name, uint access, uint share, IntPtr sec, uint disp, uint flags, IntPtr templ);
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool WriteFile(IntPtr h, byte[] buf, int len, out int written, IntPtr ov);
    [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);

    public class Dev
    {
        public string Path;
        public ushort Vid;
        public ushort Pid;
        public int OutLen;
    }

    // Enumere les interfaces HID d'un vendeur donne, ne gardant que celles
    // capables d'emettre des rapports de sortie de 64 octets.
    public static List<Dev> Find(ushort vid)
    {
        var result = new List<Dev>();
        Guid g;
        HidD_GetHidGuid(out g);
        IntPtr set = SetupDiGetClassDevs(ref g, IntPtr.Zero, IntPtr.Zero, DIGCF_PRESENT | DIGCF_DEVICEINTERFACE);
        if (set == IntPtr.Zero || set.ToInt64() == -1) return result;

        try
        {
            var dia = new SP_DEVICE_INTERFACE_DATA();
            dia.cbSize = Marshal.SizeOf(typeof(SP_DEVICE_INTERFACE_DATA));

            for (int i = 0; SetupDiEnumDeviceInterfaces(set, IntPtr.Zero, ref g, i, ref dia); i++)
            {
                int need;
                SetupDiGetDeviceInterfaceDetail(set, ref dia, IntPtr.Zero, 0, out need, IntPtr.Zero);
                if (need <= 0) continue;

                IntPtr detail = Marshal.AllocHGlobal(need);
                try
                {
                    Marshal.WriteInt32(detail, (IntPtr.Size == 8) ? 8 : 6);
                    int dummy;
                    if (!SetupDiGetDeviceInterfaceDetail(set, ref dia, detail, need, out dummy, IntPtr.Zero)) continue;
                    string path = Marshal.PtrToStringUni(new IntPtr(detail.ToInt64() + 4));
                    if (path == null) continue;

                    // Acces 0 : suffit pour interroger les attributs et reussit
                    // meme quand une autre application detient le peripherique.
                    IntPtr h = CreateFile(path, 0,
                                          FILE_SHARE_READ | FILE_SHARE_WRITE,
                                          IntPtr.Zero, OPEN_EXISTING, 0, IntPtr.Zero);
                    if (h.ToInt64() == -1) continue;

                    try
                    {
                        var attr = new HIDD_ATTRIBUTES();
                        attr.Size = Marshal.SizeOf(typeof(HIDD_ATTRIBUTES));
                        if (!HidD_GetAttributes(h, ref attr)) continue;
                        if (attr.VendorID != vid) continue;

                        IntPtr pp;
                        if (!HidD_GetPreparsedData(h, out pp)) continue;
                        HIDP_CAPS caps;
                        HidP_GetCaps(pp, out caps);
                        HidD_FreePreparsedData(pp);

                        var d = new Dev();
                        d.Path = path; d.Vid = attr.VendorID; d.Pid = attr.ProductID;
                        d.OutLen = caps.OutputReportByteLength;
                        result.Add(d);
                    }
                    finally { CloseHandle(h); }
                }
                finally { Marshal.FreeHGlobal(detail); }
            }
        }
        finally { SetupDiDestroyDeviceInfoList(set); }
        return result;
    }

    // payload = 64 octets tels qu'observes sur le fil.
    //
    // outLen vaut 64 et non 65 : le premier octet de la trame (0x2a, 0x22, 0x62)
    // est donc l'identifiant de rapport HID lui-meme. On ecrit la charge utile
    // telle quelle. Si le peripherique refuse, on retente avec la convention
    // classique d'un octet 0x00 en tete.
    public static string Write(string path, byte[] payload)
    {
        IntPtr h = CreateFile(path, GENERIC_WRITE,
                              FILE_SHARE_READ | FILE_SHARE_WRITE,
                              IntPtr.Zero, OPEN_EXISTING, 0, IntPtr.Zero);
        if (h.ToInt64() == -1)
        {
            int e = Marshal.GetLastWin32Error();
            string hint = (e == 32) ? " (peripherique detenu par une autre application, CAM ?)" : "";
            return "CreateFile echoue, erreur " + e + hint;
        }
        try
        {
            int written;
            if (WriteFile(h, payload, payload.Length, out written, IntPtr.Zero))
                return null;
            int err1 = Marshal.GetLastWin32Error();

            byte[] alt = new byte[payload.Length + 1];
            Array.Copy(payload, 0, alt, 1, payload.Length);
            if (WriteFile(h, alt, alt.Length, out written, IntPtr.Zero))
                return null;

            return "WriteFile echoue, erreurs " + err1 + " et " + Marshal.GetLastWin32Error();
        }
        finally { CloseHandle(h); }
    }
}
'@

# --------------------------------------------------------------------------

$devs = [NzxtHid]::Find(0x1E71)
$rgbDevs = $devs | Where-Object { $_.Pid -eq 0x2019 -or $_.Pid -eq 0x2012 }

Write-Host ""
Write-Host "Interfaces HID NZXT capables d'ecrire 64 octets :" -ForegroundColor Cyan
foreach ($d in $rgbDevs) {
    "  {0:x4}:{1:x4}  outLen={2}  {3}" -f $d.Vid, $d.Pid, $d.OutLen, $d.Path
}
if (-not $rgbDevs) { throw "Aucun controleur RGB NZXT trouve." }
Write-Host ""

if ($ListOnly) { return }

# --- construction des trames ---------------------------------------------

function New-Payload { return New-Object byte[] 64 }

function Set-CouleurFixe {
    param([string] $Path, [int] $Canal, [int] $r, [int] $g, [int] $b)
    $buf = New-Payload
    $buf[0] = 0x2A; $buf[1] = 0x04
    $buf[2] = $buf[3] = [byte](1 -shl ($Canal - 1))
    $buf[4] = 0x00                      # mode couleur fixe
    $buf[5] = 0x32                      # vitesse, sans effet en mode fixe
    $buf[6] = 0x00
    $buf[7] = [byte]$g; $buf[8] = [byte]$r; $buf[9] = [byte]$b   # GRB
    $buf[56] = 0x01                     # une couleur fournie
    $buf[58] = 0x08                     # 8 LED
    $buf[59] = 0x03
    return [NzxtHid]::Write($Path, $buf)
}

function Set-LedParLed {
    param([string] $Path, [int] $Canal, [array] $Couleurs)
    $mask = [byte](1 -shl ($Canal - 1))

    $buf = New-Payload
    $buf[0] = 0x22; $buf[1] = 0x10; $buf[2] = $mask; $buf[3] = 0x00
    for ($i = 0; $i -lt 8; $i++) {
        $c = $Couleurs[$i]
        $buf[4 + $i*3]     = [byte]$c[1]   # G
        $buf[4 + $i*3 + 1] = [byte]$c[0]   # R
        $buf[4 + $i*3 + 2] = [byte]$c[2]   # B
    }
    $e = [NzxtHid]::Write($Path, $buf); if ($e) { return $e }
    if ($SansValidation) { return $null }

    $buf2 = New-Payload
    $buf2[0] = 0x22; $buf2[1] = 0x11; $buf2[2] = $mask
    $e = [NzxtHid]::Write($Path, $buf2); if ($e) { return $e }

    $buf3 = New-Payload
    $ap = 0x22,0xA0,$mask,0x00,0x01,0x00,0x00,0x08,0x00,0x00,0x80,0x00,0x32,0x00,0x00,0x01
    for ($i = 0; $i -lt $ap.Count; $i++) { $buf3[$i] = [byte]$ap[$i] }
    return [NzxtHid]::Write($Path, $buf3)
}

# --- application ----------------------------------------------------------

$rr = [int]([Math]::Round($R * $Luminosite))
$gg = [int]([Math]::Round($G * $Luminosite))
$bb = [int]([Math]::Round($B * $Luminosite))

# arc-en-ciel sur 8 LED, pour le mode par LED
$arc = @(
    @(255,0,0), @(255,128,0), @(255,255,0), @(0,255,0),
    @(0,255,255), @(0,0,255), @(128,0,255), @(255,0,255)
)

$ok = 0; $ko = 0
foreach ($d in $rgbDevs) {
    # 6 canaux sur le 2019, 3 suffisent sur les 2012 mais en ecrire 6 est sans effet
    foreach ($canal in 1..6) {
        if ($PerLed) {
            $err = Set-LedParLed -Path $d.Path -Canal $canal -Couleurs $arc
            $quoi = "LED par LED"
        } else {
            $err = Set-CouleurFixe -Path $d.Path -Canal $canal -r $rr -g $gg -b $bb
            $quoi = ("fixe rgb({0},{1},{2})" -f $rr, $gg, $bb)
        }
        if ($err) {
            Write-Host ("  {0:x4} canal {1} : ECHEC -- {2}" -f $d.Pid, $canal, $err) -ForegroundColor Red
            $ko++
        } else {
            Write-Host ("  {0:x4} canal {1} : {2}" -f $d.Pid, $canal, $quoi) -ForegroundColor Green
            $ok++
        }
        Start-Sleep -Milliseconds 40
    }
}

Write-Host ""
Write-Host ("{0} trames acceptees, {1} en echec." -f $ok, $ko) -ForegroundColor Cyan
