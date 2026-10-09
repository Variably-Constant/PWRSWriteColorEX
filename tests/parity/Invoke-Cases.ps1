# Runs each parity case against one module, in a process of its own, and writes the records each
# case produced, in order, to a CLIXML file: each write to the host with its colors, each
# warning, error and verbose message, and each object written. Parity.Tests.ps1 runs it once for
# PSWriteColorEX and once for PWRSWriteColorEX and compares the two files.
param(
    [Parameter(Mandatory)][string]$ModulePath,
    [Parameter(Mandatory)][string]$CasesFile,
    [Parameter(Mandatory)][string]$OutFile,
    [Parameter(Mandatory)][string]$TestDir
)
$ErrorActionPreference = 'Continue'
Import-Module $ModulePath -Force -WarningAction SilentlyContinue -ErrorAction Stop
$null = New-Item -ItemType Directory -Path $TestDir -Force

# A value as text, with escape codes and line ends shown
function Show-Value($Value) {
    if ($null -eq $Value) { return '<null>' }
    if ($Value -is [string]) {
        return ($Value -replace ([string][char]27), '\e' -replace "`r", '\r' -replace "`n", '\n')
    }
    if ($Value -is [System.Collections.IDictionary]) {
        return '@{' + ((@($Value.Keys) | Sort-Object | ForEach-Object { "$_=" + (Show-Value $Value[$_]) }) -join '; ') + '}'
    }
    if ($Value -is [array]) {
        return '@(' + ((@($Value) | ForEach-Object { Show-Value $_ }) -join ', ') + ')'
    }
    return "$Value"
}

# One record as a line. An error keeps its first sentence: past it, PowerShell's validation errors
# quote the module's own validation script. A failed log write keeps its frame: the reason is
# .NET's text, which differs between platforms.
function Get-RecordLine($Record) {
    if ($Record -is [System.Management.Automation.InformationRecord]) {
        $message = $Record.MessageData
        if ($message -is [System.Management.Automation.HostInformationMessage]) {
            return 'HOST fg={0} bg={1} nn={2} [{3}]' -f $message.ForegroundColor, $message.BackgroundColor, $message.NoNewLine, (Show-Value $message.Message)
        }
        return 'INFO ' + (Show-Value $message)
    }
    if ($Record -is [System.Management.Automation.WarningRecord]) {
        return 'WARN ' + ($Record.Message -replace "(Couldn't write to log file ).*(\. Tried \(\d+/\d+\))$", '$1<reason>$2')
    }
    if ($Record -is [System.Management.Automation.VerboseRecord]) { return 'VERBOSE ' + $Record.Message }
    if ($Record -is [System.Management.Automation.DebugRecord]) { return 'DEBUG ' + $Record.Message }
    if ($Record -is [System.Management.Automation.ErrorRecord]) {
        $text = $Record.Exception.Message -replace "`r?`n", ' '
        if ($text -match '^(.*?\.)(\s|$)') { $text = $Matches[1] }
        return 'ERROR ' + $text
    }
    if ($null -eq $Record) { return 'OUT <null>' }
    return 'OUT ' + $Record.GetType().Name + ' ' + (Show-Value $Record)
}

# One result per case, in the order of the file: cases that differ only in letter case, such as
# two spellings of a hex code, are cases of their own
$cases = Get-Content -LiteralPath $CasesFile -Encoding UTF8 | Where-Object { $_ -and $_ -notmatch '^\s*#' }
$results = foreach ($case in $cases) {
    $records = try {
        & ([scriptblock]::Create($case)) *>&1
    } catch {
        $_
    }
    [pscustomobject]@{
        Case = $case
        Records = [string[]]@(foreach ($record in $records) { Get-RecordLine $record })
    }
}
Export-Clixml -LiteralPath $OutFile -InputObject @($results)
