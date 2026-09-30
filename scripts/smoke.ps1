# Сквозной тест ядра: открывает своё окошко с полем ввода, жмёт Alt+1 и
# Ctrl+Alt+2 так же, как человек, и проверяет, что вставилось и что буфер
# обмена вернулся. Key Manager должен быть запущен со стартовыми правилами.
#
# Безопасность: клавиши посылаются, только если в фокусе именно окно теста.
# Если Windows не дала ему фокус — тест останавливается, а не печатает
# в чужое окно.

Add-Type -AssemblyName System.Windows.Forms
Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class Kb {
  [StructLayout(LayoutKind.Sequential)]
  public struct KI { public ushort vk; public ushort scan; public uint flags; public uint time; public IntPtr extra; }
  [StructLayout(LayoutKind.Explicit, Size = 40)]
  public struct IN { [FieldOffset(0)] public uint type; [FieldOffset(8)] public KI ki; }
  [DllImport("user32.dll")] static extern uint SendInput(uint n, IN[] i, int size);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  static IN K(ushort vk, bool up) { var x = new IN(); x.type = 1; x.ki.vk = vk; x.ki.flags = up ? 2u : 0u; return x; }
  public static uint Chord(ushort[] mods, ushort key) {
    var l = new List<IN>();
    foreach (var m in mods) l.Add(K(m, false));
    l.Add(K(key, false)); l.Add(K(key, true));
    for (int i = mods.Length - 1; i >= 0; i--) l.Add(K(mods[i], true));
    return SendInput((uint)l.Count, l.ToArray(), Marshal.SizeOf(typeof(IN)));
  }
}
"@

$expected1 = 'Это тестовый текст от Key Manager.'
$oldClip = 'СТАРЫЙ БУФЕР ' + (Get-Random)
$results = New-Object System.Collections.Generic.List[string]
$failed = $false

$form = New-Object System.Windows.Forms.Form
$form.Text = 'Key Manager — автотест'
$form.Width = 520; $form.Height = 260; $form.TopMost = $true
$form.StartPosition = 'CenterScreen'
$box = New-Object System.Windows.Forms.TextBox
$box.Multiline = $true; $box.Dock = 'Fill'
$form.Controls.Add($box)

function Wait-Ui([int]$ms) {
  $sw = [Diagnostics.Stopwatch]::StartNew()
  while ($sw.ElapsedMilliseconds -lt $ms) { [System.Windows.Forms.Application]::DoEvents(); Start-Sleep -Milliseconds 15 }
}

function Ensure-Focus {
  for ($i = 0; $i -lt 20; $i++) {
    if ([Kb]::GetForegroundWindow() -eq $form.Handle) { $box.Focus() | Out-Null; return $true }
    # Windows отдаёт фокус только процессу, от которого пришло последнее
    # нажатие. Пустая клавиша 0xE8 ни на что не влияет, но делает нас им.
    [Kb]::Chord(@(), 0xE8) | Out-Null
    $form.Activate(); [Kb]::SetForegroundWindow($form.Handle) | Out-Null; Wait-Ui 100
  }
  return $false
}

$form.Add_Shown({
  try {
    [System.Windows.Forms.Clipboard]::SetText($oldClip)
    if (-not (Ensure-Focus)) { throw 'окно теста не получило фокус — клавиши не посылаю' }

    # 1. Alt+1 → короткий текст печатью
    [Kb]::Chord(@(0xA4), 0x31) | Out-Null
    Wait-Ui 1500
    if ($box.Text -eq $expected1) { $results.Add('OK   Alt+1 вставил тестовый текст') }
    else { $script:failed = $true; $results.Add("FAIL Alt+1: в поле «$($box.Text)»") }

    # 2. Ctrl+Alt+2 → многострочный текст через буфер + буфер вернулся
    $box.Clear()
    if (-not (Ensure-Focus)) { throw 'окно теста потеряло фокус' }
    [Kb]::Chord(@(0xA2, 0xA4), 0x32) | Out-Null
    Wait-Ui 2000
    $t = $box.Text
    if ($t.StartsWith('Первая строка') -and $t.Contains("`r`nВторая строка")) { $results.Add('OK   Ctrl+Alt+2 вставил многострочный текст') }
    else { $script:failed = $true; $results.Add("FAIL Ctrl+Alt+2: в поле «$t»") }
    $clip = [System.Windows.Forms.Clipboard]::GetText()
    if ($clip -eq $oldClip) { $results.Add('OK   буфер обмена вернулся') }
    else { $script:failed = $true; $results.Add("FAIL буфер обмена: «$clip»") }

    # 3. Обычная клавиша без модификаторов проходит насквозь
    $box.Clear()
    [Kb]::Chord(@(), 0x51) | Out-Null   # Q
    Wait-Ui 400
    if ($box.Text -eq 'q' -or $box.Text -eq 'й') { $results.Add('OK   обычный набор не перехватывается') }
    else { $script:failed = $true; $results.Add("FAIL обычная клавиша: «$($box.Text)»") }
  } catch {
    $script:failed = $true; $results.Add("FAIL $_")
  } finally {
    $form.Close()
  }
})

[System.Windows.Forms.Application]::Run($form)
$results | ForEach-Object { Write-Output $_ }
if ($failed) { exit 1 } else { Write-Output 'Все проверки пройдены'; exit 0 }
