param(
    [Parameter(Mandatory = $true)]
    [string] $ReadyFile,
    [int] $DurationSeconds = 15
)

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms

$form = New-Object System.Windows.Forms.Form
$form.Text = 'CrossWarp WGC animated capture fixture'
$form.StartPosition = 'Manual'
$form.Size = New-Object System.Drawing.Size(640, 420)
$form.TopMost = $true
$form.FormBorderStyle = [System.Windows.Forms.FormBorderStyle]::FixedSingle
$form.Location = [System.Drawing.Point]::new(
    [Math]::Max(0, [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea.Width - 660),
    [Math]::Max(0, [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea.Height - 440)
)

$panel = New-Object System.Windows.Forms.Panel
$panel.Dock = [System.Windows.Forms.DockStyle]::Fill
$form.Controls.Add($panel)
$script:ticks = 0
$timer = New-Object System.Windows.Forms.Timer
$timer.Interval = 33
$timer.Add_Tick({
    $script:ticks++
    $r = ($script:ticks * 29) % 256
    $g = ($script:ticks * 53) % 256
    $b = ($script:ticks * 97) % 256
    $panel.BackColor = [System.Drawing.Color]::FromArgb($r, $g, $b)
    $panel.Invalidate()
})
$closeTimer = New-Object System.Windows.Forms.Timer
$closeTimer.Interval = [Math]::Max(1, $DurationSeconds) * 1000
$closeTimer.Add_Tick({ $form.Close() })
$form.Add_Shown({
    [System.IO.File]::WriteAllText($ReadyFile, ('0x{0:X}' -f $form.Handle.ToInt64()))
    $timer.Start()
    $closeTimer.Start()
})
$form.Add_FormClosed({
    $timer.Stop()
    $closeTimer.Stop()
    $timer.Dispose()
    $closeTimer.Dispose()
})
[System.Windows.Forms.Application]::Run($form)
