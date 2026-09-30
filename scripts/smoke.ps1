# Сквозной тест Key Manager: жмёт сочетания так же, как человек, в своём
# окошке с полем ввода и проверяет результат.
#
# Тест не зависит от ваших правил: на время проверки дописывает в
# config.yaml свои (на редких сочетаниях Ctrl+Alt+Shift+F7…F11) и потом
# возвращает файл как был — даже если тест упал. Key Manager должен быть
# запущен.
#
# Безопасность: клавиши посылаются, только если в фокусе окно теста.
# Если фокус ушёл (человек переключил окно) — проверка помечается как
# недостоверная, а не как сбой.

param([string]$Config = "$env:APPDATA\app.keymanager.desktop\config.yaml")

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
  static IN K(ushort vk, bool up) { var x = new IN(); x.type = 1; x.ki.vk = vk; x.ki.flags = up ? 2u : 0u; return x; }
  public static uint Chord(ushort[] mods, ushort key) {
    var l = new List<IN>();
    foreach (var m in mods) l.Add(K(m, false));
    l.Add(K(key, false)); l.Add(K(key, true));
    for (int i = mods.Length - 1; i >= 0; i--) l.Add(K(mods[i], true));
    return SendInput((uint)l.Count, l.ToArray(), Marshal.SizeOf(typeof(IN)));
  }
  // Напечатать строку юникодом (так шлют текст голосовой ввод и другие
  // программы): у таких нажатий нет кода клавиши, символ — в скан-коде.
  public static uint TypeUnicode(string s) {
    var l = new List<IN>();
    foreach (char c in s) {
      var d = new IN(); d.type = 1; d.ki.scan = c; d.ki.flags = 4u; l.Add(d);
      var u = new IN(); u.type = 1; u.ki.scan = c; u.ki.flags = 6u; l.Add(u);
    }
    return SendInput((uint)l.Count, l.ToArray(), Marshal.SizeOf(typeof(IN)));
  }
  public static void Keys(ushort[] vks) {
    foreach (var vk in vks) Chord(new ushort[0], vk);
  }
  public static string ForegroundInfo() {
    IntPtr h = GetForegroundWindow(); uint pid; GetWindowThreadProcessId(h, out pid);
    string name = "?"; try { name = System.Diagnostics.Process.GetProcessById((int)pid).ProcessName; } catch {}
    return name + ".exe";
  }
}
"@

if (-not (Test-Path $Config)) { "Нет файла настроек: $Config"; exit 1 }
$original = [IO.File]::ReadAllText($Config, [Text.Encoding]::UTF8)

$testRules = @'

  - name: __тест короткий
    hotkey: Ctrl+Alt+Shift+F7
    text: "Короткий тестовый текст."

  - name: __тест длинный
    hotkey: Ctrl+Alt+Shift+F8
    text: |
      Первая строка длинного тестового текста.
      Вторая строка — многострочная вставка через буфер обмена.

  - name: __тест подстановок
    hotkey: Ctrl+Alt+Shift+F9
    method: type
    text: "Дата {date}, курсор[{cursor}]"

  - name: __тест клавиш
    hotkey: Ctrl+Alt+Shift+F10
    keys: "ab{Left}X{End}{Enter}{Tab 2}z"

  - name: __тест сокращения сразу
    abbr: ";кмт"
    text: "СРАЗУ"

  - name: __тест сокращения словом
    abbr: "7788"
    text: "СЛОВО"

  - name: __тест цепочки
    hotkey: Ctrl+Alt+Shift+F11
    steps:
      - text: "один"
      - wait: 100
      - keys: "{Enter}два"
'@

$results = New-Object System.Collections.Generic.List[string]
$failed = $false
$unsure = $false
$CAS = [UInt16[]]@(0xA2, 0xA4, 0xA0)   # Ctrl+Alt+Shift

try {
  # Правила дописываем в конец списка snippets (он последний в файле) —
  # с тем же отступом, что у остальных: руками файл пишут с отступом,
  # а программа при сохранении из окна — без него.
  $indent = [regex]::Match($original, '(?m)^( *)- name:').Groups[1].Value
  $rules = ($testRules -split "`r?`n" | ForEach-Object { if ($_ -match '^  ') { $indent + $_.Substring(2) } else { $_ } }) -join "`n"
  $body = $original.TrimEnd()
  if ($body -notmatch '(?m)^snippets:') { $body += "`nsnippets:" }
  [IO.File]::WriteAllText($Config, $body + "`n" + $rules + "`n", (New-Object Text.UTF8Encoding($false)))
  Start-Sleep -Milliseconds 2500   # Key Manager перечитывает файл раз в секунду

  $form = New-Object System.Windows.Forms.Form
  $form.Text = 'Key Manager — автотест'
  $form.Width = 520; $form.Height = 260; $form.TopMost = $true
  $form.StartPosition = 'CenterScreen'
  $box = New-Object System.Windows.Forms.TextBox
  $box.Multiline = $true; $box.AcceptsReturn = $true; $box.AcceptsTab = $true; $box.Dock = 'Fill'
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
  # $send — либо сочетание (mods + key), либо свой блок отправки клавиш.
  function Step([string]$name, [UInt16[]]$mods, [UInt16]$key, [int]$waitMs, [scriptblock]$verify, [scriptblock]$send = $null) {
    # Фокус однажды уже уходил — человек работает за компьютером. Дальше
    # клавиши не шлём вовсе: следующее сочетание могло бы напечатать в его окно.
    if ($script:unsure) { $results.Add("SKIP $name — тест остановлен"); return }
    $box.Clear()
    if (-not (Ensure-Focus)) { $script:unsure = $true; $results.Add("SKIP $name — окно теста не получило фокус, клавиши не посылал"); return }
    if ($send) { & $send } else { [Kb]::Chord($mods, $key) | Out-Null }
    Wait-Ui $waitMs
    if ([Kb]::GetForegroundWindow() -ne $form.Handle) { $script:unsure = $true; $results.Add("SKIP $name — фокус ушёл к $([Kb]::ForegroundInfo())"); return }
    $err = & $verify
    if ($err) { $script:failed = $true; $results.Add("FAIL $name — $err") } else { $results.Add("OK   $name") }
  }
  function Show([string]$s) { $s -replace "`t", '\t' -replace "`r`n", '\n' }

  $oldClip = 'СТАРЫЙ БУФЕР ' + (Get-Random)
  $today = (Get-Date).ToString('dd.MM.yyyy')

  $form.Add_Shown({
    try {
      [System.Windows.Forms.Clipboard]::SetText($oldClip)
      Step 'короткий текст печатью' $CAS 0x76 1500 {
        if ($box.Text -ne 'Короткий тестовый текст.') { "в поле «$($box.Text)»" }
      }
      Step 'многострочный текст через буфер, буфер вернулся' $CAS 0x77 2000 {
        $t = $box.Text
        if (-not ($t.StartsWith('Первая строка') -and $t.Contains("`r`nВторая строка"))) { "в поле «$(Show $t)»" }
        else {
          $clip = [System.Windows.Forms.Clipboard]::GetText()
          if ($clip -ne $oldClip) { "буфер обмена не вернулся: «$clip»" }
        }
      }
      Step 'подстановка {date} и {cursor}' $CAS 0x78 1500 {
        $want = "Дата $today, курсор[]"
        if ($box.Text -ne $want) { "в поле «$($box.Text)», ждал «$want»" }
        elseif ($box.SelectionStart -ne $want.Length - 1) { "курсор на позиции $($box.SelectionStart), ждал $($want.Length - 1)" }
      }
      Step 'клавиши {Left} {End} {Enter} {Tab 2}' $CAS 0x79 1500 {
        if ($box.Text -ne "aXb`r`n`t`tz") { "в поле «$(Show $box.Text)»" }
      }
      Step 'цепочка: текст → пауза → клавиши' $CAS 0x7A 1800 {
        if ($box.Text -ne "один`r`nдва") { "в поле «$(Show $box.Text)»" }
      }
      Step 'сокращение «сразу» (;кмт, юникодом)' $null 0 1500 {
        if ($box.Text -ne 'СРАЗУ') { "в поле «$($box.Text)»" }
      } { [Kb]::TypeUnicode(';кмт') | Out-Null }
      Step 'сокращение «после пробела» (7788 + пробел, клавишами)' $null 0 1500 {
        if ($box.Text -ne 'СЛОВО ') { "в поле «$($box.Text)»" }
      } { [Kb]::Keys([UInt16[]]@(0x37, 0x37, 0x38, 0x38, 0x20)) }
      Step 'сокращение внутри слова не срабатывает (57788 + пробел)' $null 0 1000 {
        if ($box.Text -ne '57788 ') { "в поле «$($box.Text)»" }
      } { [Kb]::Keys([UInt16[]]@(0x35, 0x37, 0x37, 0x38, 0x38, 0x20)) }
      Step 'обычный набор не перехватывается' ([UInt16[]]@()) 0x51 400 {
        if ($box.Text -ne 'q' -and $box.Text -ne 'й') { "в поле «$($box.Text)»" }
      }
    } catch { $script:failed = $true; $results.Add("FAIL $_") }
    finally { $form.Close() }
  })
  [System.Windows.Forms.Application]::Run($form)
}
finally {
  [IO.File]::WriteAllText($Config, $original, (New-Object Text.UTF8Encoding($false)))
}

$results | ForEach-Object { Write-Output $_ }
if ($failed) { exit 1 }
if ($unsure) { Write-Output 'Часть проверок пропущена: во время теста не трогайте клавиатуру и мышь'; exit 2 }
Write-Output 'Все проверки пройдены'; exit 0
