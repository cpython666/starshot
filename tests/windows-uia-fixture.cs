// Original native UI only. No screenshots, clipboard, input injection or app text reads.
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.IO;
using System.Runtime.InteropServices;
using System.Threading;
using System.Web.Script.Serialization;
using System.Windows.Forms;
using W=System.Windows;
using C=System.Windows.Controls;
using A=System.Windows.Automation.Peers;

class BlockingButton:C.Button {
 public static string BlockFile;
 protected override A.AutomationPeer OnCreateAutomationPeer(){return new Peer(this);}
 class Peer:A.ButtonAutomationPeer {
  public Peer(BlockingButton b):base(b){}
  protected override W.Rect GetBoundingRectangleCore(){
   if(File.Exists(BlockFile))Thread.Sleep(Timeout.Infinite);
   return base.GetBoundingRectangleCore();
  }
 }
}
class FixtureForm:Form {public void RebuildHandle(){RecreateHandle();}}
class Fixture {
 [DllImport("user32.dll")] static extern bool SetProcessDpiAwarenessContext(IntPtr value);
 [DllImport("user32.dll")] static extern IntPtr SetThreadDpiAwarenessContext(IntPtr value);
 [DllImport("user32.dll")] static extern IntPtr GetThreadDpiAwarenessContext();
 [DllImport("user32.dll")] static extern bool AreDpiAwarenessContextsEqual(IntPtr a,IntPtr b);
 [DllImport("user32.dll")] static extern void DisableProcessWindowsGhosting();
 [DllImport("user32.dll")] static extern uint GetDpiForWindow(IntPtr h);
 [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h,out R rect);
 [StructLayout(LayoutKind.Sequential)] struct R {public int left,top,right,bottom;}
 static object Rect(Rectangle r){return new{x=r.X,y=r.Y,width=r.Width,height=r.Height};}
 static void Save(string path,object value){File.WriteAllText(path,new JavaScriptSerializer().Serialize(value));}
 [STAThread] static void Main(string[] args){
  bool dpiSet=SetProcessDpiAwarenessContext(new IntPtr(-4));
  var previousDpi=SetThreadDpiAwarenessContext(new IntPtr(-4));
  DisableProcessWindowsGhosting(); // Fault injection affects only this original fixture.
  string directory=Path.GetFullPath(args[0]);Directory.CreateDirectory(directory);
  int monitor=int.Parse(args[1]);var screen=Screen.AllScreens[monitor];
  BlockingButton.BlockFile=Path.Combine(directory,"block");
  var f=new FixtureForm{Text="Starshot original native acceptance",AutoScaleMode=AutoScaleMode.None,StartPosition=FormStartPosition.Manual,
   Location=new Point(screen.Bounds.X+(args.Length>2?int.Parse(args[2]):80),screen.Bounds.Y+80),ClientSize=new Size(780,580),TopMost=true};
  var button=new Button{Text="Original native button",Bounds=new Rectangle(30,30,200,45)};
  var edit=new TextBox{Multiline=true,Bounds=new Rectangle(260,30,200,45)};
  var custom=new Panel{Bounds=new Rectangle(520,30,200,90)};
  custom.Paint+=(o,e)=>e.Graphics.FillRectangle(Brushes.Coral,25,20,140,45);
  var host=new System.Windows.Forms.Integration.ElementHost{Bounds=new Rectangle(30,110,700,420)};
  var canvas=new C.Canvas{Background=W.Media.Brushes.WhiteSmoke,UseLayoutRounding=true};
  var labeled=new C.Button{Content="Original button label",Width=210,Height=42};
  C.Canvas.SetLeft(labeled,20);C.Canvas.SetTop(labeled,20);canvas.Children.Add(labeled);
  var larger=new C.Button{Content="Back",Width=210,Height=70};C.Canvas.SetLeft(larger,300);C.Canvas.SetTop(larger,20);canvas.Children.Add(larger);
  var overlap=new C.Button{Content="Front",Width=130,Height=45};C.Canvas.SetLeft(overlap,330);C.Canvas.SetTop(overlap,35);canvas.Children.Add(overlap);
  var stack=new C.StackPanel();var scrollButtons=new List<C.Button>();
  for(int i=0;i<8;i++){var b=new C.Button{Content="Original row "+i,Height=46,Width=180};stack.Children.Add(b);scrollButtons.Add(b);}
  var scroll=new C.ScrollViewer{Content=stack,Width=220,Height=160,VerticalScrollBarVisibility=C.ScrollBarVisibility.Visible};
  C.Canvas.SetLeft(scroll,20);C.Canvas.SetTop(scroll,130);canvas.Children.Add(scroll);
  var blocker=new BlockingButton{Content="Original blocking provider",Width=210,Height=45};C.Canvas.SetLeft(blocker,300);C.Canvas.SetTop(blocker,180);canvas.Children.Add(blocker);
  host.Child=canvas;f.Controls.AddRange(new Control[]{button,edit,custom,host});
  int revision=0;string last="";
  Action publish=()=>{
   var points=new List<object>();
   Action<string,Control> native=(id,c)=>{R r;GetWindowRect(c.Handle,out r);var b=Rectangle.FromLTRB(r.left,r.top,r.right,r.bottom);points.Add(new{id=id,x=b.X+b.Width/2,y=b.Y+b.Height/2,expected=Rect(b)});};
   Action<string,W.FrameworkElement> wpf=(id,c)=>{var a=c.PointToScreen(new W.Point(0,0));var b=c.PointToScreen(new W.Point(c.ActualWidth,c.ActualHeight));var r=Rectangle.FromLTRB((int)Math.Round(a.X),(int)Math.Round(a.Y),(int)Math.Round(b.X),(int)Math.Round(b.Y));points.Add(new{id=id,x=r.X+r.Width/2,y=r.Y+r.Height/2,expected=Rect(r)});};
   native("native-button",button);native("edit",edit);wpf("labeled-button",labeled);wpf("overlap",overlap);
   wpf("scroll-button",scrollButtons[revision==1?2:0]);wpf("blocker",blocker);
   var paint=custom.RectangleToScreen(new Rectangle(25,20,140,45));points.Add(new{id="paint-only",x=paint.X+70,y=paint.Y+22,expected=Rect(paint)});
   Save(Path.Combine(directory,"fixture.json"),new{pid=Process.GetCurrentProcess().Id,hwnd=f.Handle.ToInt64(),monitor=monitor,dpi=GetDpiForWindow(f.Handle),dpiSet=dpiSet,threadDpiSet=previousDpi!=IntPtr.Zero,pmv2=AreDpiAwarenessContextsEqual(GetThreadDpiAwarenessContext(),new IntPtr(-4)),screen=Rect(screen.Bounds),revision=revision,command=last,points=points});
  };
  var timer=new System.Windows.Forms.Timer{Interval=200};bool first=true;
  timer.Tick+=(o,e)=>{
   if(first){first=false;publish();}
   string file=Path.Combine(directory,"command.txt");
   if(!File.Exists(file))return;string command=File.ReadAllText(file).Trim();if(command==last)return;last=command;
   if(command=="scroll"){scroll.ScrollToVerticalOffset(92);revision=1;f.BeginInvoke((Action)(()=>{scroll.UpdateLayout();publish();}));}
   if(command=="rebuild" || command.StartsWith("rebuild:")){f.RebuildHandle();revision=2;f.BeginInvoke(publish);}
   if(command.StartsWith("front:")){f.Show();f.TopMost=true;f.BringToFront();f.Activate();publish();}
   if(command.StartsWith("hide:")){f.Hide();publish();}
   if(command=="quit")f.Close();
  };
  f.Shown+=(o,e)=>timer.Start();Application.Run(f);
 }
}
