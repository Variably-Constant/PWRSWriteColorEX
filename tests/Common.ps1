# Shared by every suite: imports the module cargo pwrs built, which the runner names in
# PWRS_MODULE, and reads what Write-ColorEX writes to the host.

function Import-PWRSWriteColorEX {
    if (-not $env:PWRS_MODULE) {
        throw 'PWRS_MODULE is not set; run the suites through cargo pwrs test'
    }
    Import-Module (Join-Path $env:PWRS_MODULE 'PWRSWriteColorEX.psd1') -Force -ErrorAction Stop
}

# The escape character, which Windows PowerShell 5.1 has no `e for
$script:esc = [char]27

# Each write a script block makes to the host, as Write-Host makes it: the text, the colors and
# whether the line stays open.
function Get-HostWrite {
    param([Parameter(Mandatory)][scriptblock]$Script)
    foreach ($record in (& $Script 6>&1)) {
        if ($record -is [System.Management.Automation.InformationRecord] -and
            $record.MessageData -is [System.Management.Automation.HostInformationMessage]) {
            $message = $record.MessageData
            [pscustomobject]@{
                Text = $message.Message
                ForegroundColor = $message.ForegroundColor
                BackgroundColor = $message.BackgroundColor
                NoNewLine = $message.NoNewLine
            }
        }
    }
}

# Removes escape codes, to compare the text alone.
function Remove-EscapeCode {
    param([string]$Text)
    $Text -replace "$([char]27)\[[0-9;]*m", ''
}

# The text of each write a script block makes to the host.
function Get-HostText {
    param([Parameter(Mandatory)][scriptblock]$Script)
    @(Get-HostWrite $Script | ForEach-Object { $_.Text })
}

# The variables the color detection reads, besides TERM
$script:colorVariables = @(
    'FORCE_COLOR', 'NO_COLOR', 'CLICOLOR', 'CLICOLOR_FORCE', 'COLORTERM', 'WT_SESSION', 'TERM_PROGRAM', 'TMUX', 'VTE_VERSION',
    'KONSOLE_VERSION', 'KITTY_WINDOW_ID', 'ALACRITTY_WINDOW_ID', 'TERMINAL_EMULATOR', 'ConEmuANSI'
)

# Sets the color variables a test needs and clears the others. FORCE_COLOR 1, 2 and 3 make
# Write-ColorEX write ANSI4, ANSI8 and TrueColor whatever the terminal, NO_COLOR and TERM=dumb
# turn colors off. The module is imported again so its detection reads the new values.
function Set-ColorEnvironment {
    param([hashtable]$Values = @{})
    foreach ($name in $script:colorVariables) {
        [System.Environment]::SetEnvironmentVariable($name, $null)
    }
    [System.Environment]::SetEnvironmentVariable('TERM', 'xterm-256color')
    foreach ($name in $Values.Keys) {
        [System.Environment]::SetEnvironmentVariable($name, $Values[$name])
    }
    Import-PWRSWriteColorEX
}

# The color variables as they were, to put back after a suite.
function Save-ColorEnvironment {
    $saved = @{}
    foreach ($name in $script:colorVariables + 'TERM') {
        $saved[$name] = [System.Environment]::GetEnvironmentVariable($name)
    }
    $saved
}

function Restore-ColorEnvironment {
    param([hashtable]$Saved)
    foreach ($name in $Saved.Keys) {
        [System.Environment]::SetEnvironmentVariable($name, $Saved[$name])
    }
}

# What a script block writes: the text of each write to the host, and each warning.
function Invoke-Captured {
    param([Parameter(Mandatory)][scriptblock]$Script)
    $text = [System.Collections.Generic.List[string]]::new()
    $warnings = [System.Collections.Generic.List[string]]::new()
    foreach ($record in (& $Script *>&1)) {
        if ($record -is [System.Management.Automation.WarningRecord]) {
            $warnings.Add($record.Message)
        } elseif ($record -is [System.Management.Automation.InformationRecord] -and
            $record.MessageData -is [System.Management.Automation.HostInformationMessage]) {
            $text.Add($record.MessageData.Message)
        }
    }
    [pscustomobject]@{ Text = $text.ToArray(); Warnings = $warnings.ToArray() }
}
