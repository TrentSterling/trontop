param([Parameter(Mandatory=$true)][int]$ProcessId, [int]$Seconds=20, [string]$OutputPath='')
$ErrorActionPreference='Stop'
if($Seconds -lt 1 -or $Seconds -gt 60){throw 'Seconds must be between 1 and 60'}
Get-Process -Id $ProcessId -ErrorAction Stop | Out-Null
Add-Type @'
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Threading;
public static class TrontDragProbe {
 [StructLayout(LayoutKind.Sequential)] public struct RECT {public int left,top,right,bottom;}
 [StructLayout(LayoutKind.Sequential)] public struct POINT {public int x,y;}
 [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);
 [DllImport("user32.dll")] static extern bool GetCursorPos(out POINT p);
 [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h,out RECT r);
 [DllImport("user32.dll")] static extern short GetAsyncKeyState(int key);
 [DllImport("user32.dll")] static extern IntPtr SetThreadDpiAwarenessContext(IntPtr c);
 [DllImport("winmm.dll")] static extern uint timeBeginPeriod(uint p);
 [DllImport("winmm.dll")] static extern uint timeEndPeriod(uint p);
 public sealed class Sample {public double ms;public int gesture,cursorX,cursorY,windowX,windowY;}
 public static List<Sample> Measure(uint target,int seconds) {
  var old=SetThreadDpiAwarenessContext(new IntPtr(-4));
  var rows=new List<Sample>();var timer=Stopwatch.StartNew();bool wasDown=false;int gesture=0;
  timeBeginPeriod(1);
  try {
   while(timer.Elapsed.TotalSeconds<seconds){
    var h=GetForegroundWindow();uint p;GetWindowThreadProcessId(h,out p);
    bool down=p==target&&(GetAsyncKeyState(1)&0x8000)!=0;
    if(down){
     if(!wasDown)gesture++;
     POINT c;RECT r;GetCursorPos(out c);GetWindowRect(h,out r);
     rows.Add(new Sample{ms=timer.Elapsed.TotalMilliseconds,gesture=gesture,cursorX=c.x,cursorY=c.y,windowX=r.left,windowY=r.top});
    }
    wasDown=down;Thread.Sleep(4);
   }
  }finally{timeEndPeriod(1);SetThreadDpiAwarenessContext(old);}
  return rows;
 }
}
'@
Write-Output "Recording $Seconds seconds. Drag the target window normally with your mouse. No input is injected."
$samples=[TrontDragProbe]::Measure($ProcessId,$Seconds)
if($OutputPath){$samples | Select-Object ms,gesture,cursorX,cursorY,windowX,windowY | Export-Csv -LiteralPath $OutputPath -NoTypeInformation}
$valid=0
foreach($group in ($samples | Group-Object gesture)){
 $rows=@($group.Group)
 if($rows.Count -lt 10){continue}
 $moves=0;$lastMove=$rows[0].ms;$gaps=[Collections.Generic.List[double]]::new();$errors=[Collections.Generic.List[double]]::new()
 $anchorX=$rows[0].cursorX-$rows[0].windowX;$anchorY=$rows[0].cursorY-$rows[0].windowY
 for($i=1;$i -lt $rows.Count;$i++){
  $a=$rows[$i-1];$b=$rows[$i]
  if($b.windowX -ne $a.windowX -or $b.windowY -ne $a.windowY){
   $moves++;$gaps.Add($b.ms-$lastMove);$lastMove=$b.ms
  }
  if($b.ms-$rows[0].ms -gt 150){
   $dx=$b.cursorX-$b.windowX-$anchorX;$dy=$b.cursorY-$b.windowY-$anchorY
   $errors.Add([Math]::Sqrt($dx*$dx+$dy*$dy))
  }
 }
 if($moves -lt 5){continue}
 $valid++
 $sorted=@($gaps | Sort-Object)
 [pscustomobject]@{
  Gesture=$group.Name
  DurationSeconds=[Math]::Round(($rows[-1].ms-$rows[0].ms)/1000,3)
  WindowPositionChanges=$moves
  MedianPositionGapMs=[Math]::Round($sorted[[int][Math]::Floor(($sorted.Count-1)*0.50)],2)
  P95PositionGapMs=[Math]::Round($sorted[[int][Math]::Floor(($sorted.Count-1)*0.95)],2)
  MaxPositionGapMs=[Math]::Round(($gaps|Measure-Object -Maximum).Maximum,2)
  MeanAnchorDeviationPx=[Math]::Round(($errors|Measure-Object -Average).Average,2)
 }
}
if($valid -eq 0){throw 'No sustained target-window drag was recorded. No smoothness conclusion can be drawn.'}
Write-Output 'Position timing is not presented-frame timing. Pauses, snapping and a changing pointer anchor can skew these diagnostics.'
