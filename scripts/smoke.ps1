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
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  public static string ForegroundInfo() {
    IntPtr h = GetForegroundWindow(); uint pid; GetWindowThreadProcessId(h, out pid);
    string name = "?"; try { name = System.Diagnostics.Process.GetProcessById((int)pid).ProcessName; } catch {}
    return name + ".exe";
  }
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
$unsure = $false

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

# Одна проверка: фокус у окна теста до и после. Если фокус ушёл (человек
# переключил окно), результат недостоверен — так и пишем, а не «сбой».
function Step([string]$name, [scriptblock]$send, [int]$waitMs, [scriptblock]$verify) {
  $box.Clear()
  if (-not (Ensure-Focus)) { $script:unsure = $true; $results.Add("SKIP $name — окно теста не получило фокус, клавиши не посылал"); return }
  & $send
  Wait-Ui $waitMs
  if ([Kb]::GetForegroundWindow() -ne $form.Handle) { $script:unsure = $true; $results.Add("SKIP $name — фокус ушёл к $([Kb]::ForegroundInfo()), результат недостоверен"); return }
  $err = & $verify
  if ($err) { $script:failed = $true; $results.Add("FAIL $name — $err") } else { $results.Add("OK   $name") }
}

$form.Add_Shown({
  try {
    [System.Windows.Forms.Clipboard]::SetText($oldClip)

    Step 'Alt+1 вставил тестовый текст' { [Kb]::Chord(@(0xA4), 0x31) | Out-Null } 1500 {
      if ($box.Text -ne $expected1) { "в поле «$($box.Text)»" }
    }
    Step 'Ctrl+Alt+2 вставил многострочный текст и вернул буфер' { [Kb]::Chord(@(0xA2, 0xA4), 0x32) | Out-Null } 2000 {
      $t = $box.Text
      if (-not ($t.StartsWith('Первая строка') -and $t.Contains("`r`nВторая строка"))) { "в поле «$t»" }
      else {
        $clip = [System.Windows.Forms.Clipboard]::GetText()
        if ($clip -ne $oldClip) { "буфер обмена не вернулся: «$clip»" }
      }
    }
    Step 'обычный набор не перехватывается' { [Kb]::Chord(@(), 0x51) | Out-Null } 400 {
      if ($box.Text -ne 'q' -and $box.Text -ne 'й') { "в поле «$($box.Text)»" }
    }
  } catch {
    $script:failed = $true; $results.Add("FAIL $_")
  } finally {
    $form.Close()
  }
})

[System.Windows.Forms.Application]::Run($form)
$results | ForEach-Object { Write-Output $_ }
if ($failed) { exit 1 }
if ($unsure) { Write-Output 'Часть проверок пропущена: во время теста не трогайте клавиатуру и мышь'; exit 2 }
Write-Output 'Все проверки пройдены'; exit 0
