# Runs a program on a Windows desktop of its own, so none of its windows can appear on the desktop in use or take
# the keyboard: a window belongs to the desktop its process started on, and only the desktop in use is drawn and
# receives input.
#
#   pwsh -NoProfile -File e2e/hidden-desktop.ps1 -Parent <pid> -Exe <program> [arguments...]
#
# Prints `pid <n>` once the program runs, waits for it and exits with its exit code. The program and everything it
# starts sit in a job that ends with this script, and the script ends them when the process named by -Parent is gone.
# Exit 3: a window of the program was found on the desktop in use, and the program was stopped at once.
param(
    [Parameter(Mandatory)][int] $Parent,
    [Parameter(Mandatory)][string] $Exe,
    [Parameter(ValueFromRemainingArguments)][string[]] $Arguments = @()
)
$ErrorActionPreference = 'Stop'

Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;

public static class HiddenDesktop
{
    const uint GENERIC_ALL = 0x10000000;
    const uint CREATE_SUSPENDED = 0x4;
    const uint CREATE_UNICODE_ENVIRONMENT = 0x400;
    const uint JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE = 0x2000;
    const int JobObjectBasicProcessIdList = 3;
    const int JobObjectExtendedLimitInformation = 9;
    const uint WAIT_TIMEOUT = 0x102;
    const uint RESUME_FAILED = 0xFFFFFFFF;
    const uint SYNCHRONIZE = 0x100000;
    public const int WINDOW_ON_DESKTOP_IN_USE = 3;

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    struct STARTUPINFO
    {
        public int cb; public string lpReserved; public string lpDesktop; public string lpTitle;
        public int dwX, dwY, dwXSize, dwYSize, dwXCountChars, dwYCountChars, dwFillAttribute, dwFlags;
        public short wShowWindow, cbReserved2;
        public IntPtr lpReserved2, hStdInput, hStdOutput, hStdError;
    }

    [StructLayout(LayoutKind.Sequential)]
    struct PROCESS_INFORMATION { public IntPtr hProcess, hThread; public int dwProcessId, dwThreadId; }

    [StructLayout(LayoutKind.Sequential)]
    struct JOBOBJECT_BASIC_LIMIT_INFORMATION
    {
        public long PerProcessUserTimeLimit, PerJobUserTimeLimit;
        public uint LimitFlags;
        public UIntPtr MinimumWorkingSetSize, MaximumWorkingSetSize;
        public uint ActiveProcessLimit;
        public UIntPtr Affinity;
        public uint PriorityClass, SchedulingClass;
    }

    [StructLayout(LayoutKind.Sequential)]
    struct IO_COUNTERS { public ulong ReadOperationCount, WriteOperationCount, OtherOperationCount, ReadTransferCount, WriteTransferCount, OtherTransferCount; }

    [StructLayout(LayoutKind.Sequential)]
    struct JOBOBJECT_EXTENDED_LIMIT_INFORMATION
    {
        public JOBOBJECT_BASIC_LIMIT_INFORMATION BasicLimitInformation;
        public IO_COUNTERS IoInfo;
        public UIntPtr ProcessMemoryLimit, JobMemoryLimit, PeakProcessMemoryUsed, PeakJobMemoryUsed;
    }

    delegate bool EnumWindowsProc(IntPtr window, IntPtr parameter);

    [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    static extern IntPtr CreateDesktopW(string name, IntPtr device, IntPtr mode, int flags, uint access, IntPtr security);
    [DllImport("user32.dll")] static extern bool CloseDesktop(IntPtr desktop);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumWindowsProc callback, IntPtr parameter);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
    [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    static extern bool CreateProcessW(string application, StringBuilder commandLine, IntPtr processSecurity, IntPtr threadSecurity, bool inherit, uint flags, IntPtr environment, string directory, ref STARTUPINFO startup, out PROCESS_INFORMATION information);
    [DllImport("kernel32.dll", SetLastError = true)] static extern IntPtr CreateJobObjectW(IntPtr security, string name);
    [DllImport("kernel32.dll", SetLastError = true)] static extern bool SetInformationJobObject(IntPtr job, int kind, ref JOBOBJECT_EXTENDED_LIMIT_INFORMATION information, int length);
    [DllImport("kernel32.dll", SetLastError = true)] static extern bool QueryInformationJobObject(IntPtr job, int kind, IntPtr information, int length, out int returned);
    [DllImport("kernel32.dll", SetLastError = true)] static extern bool AssignProcessToJobObject(IntPtr job, IntPtr process);
    [DllImport("kernel32.dll")] static extern bool TerminateJobObject(IntPtr job, uint exitCode);
    [DllImport("kernel32.dll", SetLastError = true)] static extern uint ResumeThread(IntPtr thread);
    [DllImport("kernel32.dll")] static extern bool TerminateProcess(IntPtr process, uint exitCode);
    [DllImport("kernel32.dll")] static extern uint WaitForSingleObject(IntPtr handle, uint milliseconds);
    [DllImport("kernel32.dll")] static extern bool GetExitCodeProcess(IntPtr process, out uint exitCode);
    [DllImport("kernel32.dll", SetLastError = true)] static extern IntPtr OpenProcess(uint access, bool inherit, int process);
    [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr handle);

    static HashSet<uint> ProcessesOf(IntPtr job)
    {
        var found = new HashSet<uint>();
        int size = 8 + IntPtr.Size * 1024;
        IntPtr buffer = Marshal.AllocHGlobal(size);
        try
        {
            int returned;
            if (!QueryInformationJobObject(job, JobObjectBasicProcessIdList, buffer, size, out returned)) return found;
            int count = Marshal.ReadInt32(buffer, 4);
            for (int i = 0; i < count; i++) found.Add((uint)Marshal.ReadIntPtr(buffer, 8 + i * IntPtr.Size).ToInt64());
        }
        finally { Marshal.FreeHGlobal(buffer); }
        return found;
    }

    static bool ShowsOnDesktopInUse(IntPtr job)
    {
        HashSet<uint> mine = ProcessesOf(job);
        bool shown = false;
        EnumWindows((window, parameter) =>
        {
            uint owner;
            GetWindowThreadProcessId(window, out owner);
            if (mine.Contains(owner)) shown = true;
            return !shown;
        }, IntPtr.Zero);
        return shown;
    }

    public static int Run(string commandLine, int parent)
    {
        string name = "scarlet-e2e-" + Process.GetCurrentProcess().Id;
        IntPtr desktop = CreateDesktopW(name, IntPtr.Zero, IntPtr.Zero, 0, GENERIC_ALL, IntPtr.Zero);
        if (desktop == IntPtr.Zero) throw new Win32Exception(Marshal.GetLastWin32Error(), "no desktop of its own could be made, so the program was not started");
        IntPtr job = CreateJobObjectW(IntPtr.Zero, null);
        if (job == IntPtr.Zero) throw new Win32Exception(Marshal.GetLastWin32Error(), "no job could be made");
        var limits = new JOBOBJECT_EXTENDED_LIMIT_INFORMATION();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if (!SetInformationJobObject(job, JobObjectExtendedLimitInformation, ref limits, Marshal.SizeOf(typeof(JOBOBJECT_EXTENDED_LIMIT_INFORMATION)))) throw new Win32Exception(Marshal.GetLastWin32Error(), "the job could not be set to end with this script");

        var startup = new STARTUPINFO();
        startup.cb = Marshal.SizeOf(typeof(STARTUPINFO));
        startup.lpDesktop = "WinSta0\\" + name;
        PROCESS_INFORMATION started;
        if (!CreateProcessW(null, new StringBuilder(commandLine), IntPtr.Zero, IntPtr.Zero, false, CREATE_SUSPENDED | CREATE_UNICODE_ENVIRONMENT, IntPtr.Zero, null, ref startup, out started)) throw new Win32Exception(Marshal.GetLastWin32Error(), "the program did not start");
        if (!AssignProcessToJobObject(job, started.hProcess))
        {
            int error = Marshal.GetLastWin32Error();
            TerminateProcess(started.hProcess, 1);
            throw new Win32Exception(error, "the program could not be put in the job, so it was stopped before it ran");
        }
        if (ResumeThread(started.hThread) == RESUME_FAILED)
        {
            int error = Marshal.GetLastWin32Error();
            TerminateJobObject(job, 1);
            throw new Win32Exception(error, "the program could not be resumed, so it was stopped before it ran");
        }
        Console.Out.WriteLine("pid " + started.dwProcessId);
        Console.Out.Flush();

        IntPtr watched = OpenProcess(SYNCHRONIZE, false, parent);
        int result;
        for (;;)
        {
            if (WaitForSingleObject(started.hProcess, 100) != WAIT_TIMEOUT)
            {
                uint exitCode;
                GetExitCodeProcess(started.hProcess, out exitCode);
                result = (int)exitCode;
                break;
            }
            if (ShowsOnDesktopInUse(job)) { result = WINDOW_ON_DESKTOP_IN_USE; break; }
            if (watched == IntPtr.Zero || WaitForSingleObject(watched, 0) != WAIT_TIMEOUT) { result = 1; break; }
        }
        TerminateJobObject(job, 1);
        CloseHandle(started.hThread);
        CloseHandle(started.hProcess);
        if (watched != IntPtr.Zero) CloseHandle(watched);
        CloseHandle(job);
        CloseDesktop(desktop);
        return result;
    }
}
'@

$quoted = @($Exe) + $Arguments | ForEach-Object { '"' + ($_ -replace '"', '\"') + '"' }
exit [HiddenDesktop]::Run(($quoted -join ' '), $Parent)
