/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: BattleTris.C                                        */
/*    DATE: Sat Feb 12 17:34:21 1994                            */
/****************************************************************/

#include "BTConfig.H"

#include <Xm/Xm.h>
#include <Xm/AtomMgr.h>
#include <Xm/Protocols.h>

#if HAVE_UNISTD_H
# include <unistd.h>
#endif


#include <iostream>
using namespace std;

#if STDC_HEADERS
# include <stdlib.h>
# include <string.h>
#endif

#include "SigReceiver.H"
#include "BTSigHandlers.H"
#include "BTStartup.H"
#include "BTConstants.H"
#include "BTProtocol.H"
#include "BattleTris.H"
#include "BTXDisplay.H"
#include "BTGame.H"

#ifdef __hpux
typedef char* caddr_t;
#endif

#if XtSpecificationRelease > 4
# define ARGC_PTR (int *)
#else
# define ARGC_PTR (unsigned int *)
#endif

const int BT_SUCCESS = 0;       // Exit status for successful initialization
const int BT_ERROR = 1;         // Exit status for failed initialization
const int BT_USAGE = 2;         // Exit status for invalid command-line args

int g_signalfds[2] = { -1, -1 };        // Pipe fds for indicating signals

XtAppContext g_appctx = (XtAppContext) NULL; // Application context for Xt
Display *g_display = (Display *) NULL;  // X Display pointer
Widget g_toplevel = (Widget) NULL;      // Top-level shell widget
Pixmap g_pixmap = XmUNSPECIFIED_PIXMAP; // Background, border pixmap
Colormap g_colormap = (Colormap) NULL;  // Used if PseudoColor is not default
GC g_GC = 0;
int g_depth = 24;
Window g_rootWindow = 0;
int g_screen = 0;
Visual *g_visual = 0;

BTStartup *g_startup = NULL;            // Application object
BTResources g_resources;                // Application resources

BTDisplay *DISPLAY = NULL;              // Application object
BTWidget *toplevel_ = NULL;             // Wrapper for toplevel widget

extern BTGame *GAME;

static void drop(Widget, XEvent *, char **, unsigned *) {
  GAME->beginDrop();
}

static void condor(Widget, XEvent *, char **, unsigned *) {
  GAME->condor();
}

static void pause(Widget, XEvent *, char **, unsigned *) {
  GAME->pause();
}

static void moveLeft(Widget, XEvent *, char **, unsigned *) {
  GAME->moveLeft();
}

static void moveRight(Widget, XEvent *, char **, unsigned *) {
  GAME->moveRight();
}

static void rotate(Widget, XEvent *, char **, unsigned *) {
  GAME->rotate();
}

static String def_translations =
   (String)"\"j\" : move_left()\n\
   \"l\" : move_right()\n\
   \"k\" : rotate()\n\
   \"J\" : move_left()\n\
   \"L\" : move_right()\n\
   \"K\" : rotate()\n\
   \"p\" : pause()\n\
   \"P\" : pause()\n\
   \"c\" : condor()\n\
   \"C\" : condor()\n\
   \" \" : drop()";

static XtTranslations def_translation_tab =
  XtParseTranslationTable(def_translations);

XtActionsRec def_actions[] = {
  { (char *)"move_left", moveLeft },
  { (char *)"move_right", moveRight },
  { (char *)"drop", drop },
  { (char *)"pause", pause },
  { (char *)"condor", condor },
  { (char *)"rotate", rotate }
};

static XtResource g_resdefs[] = {
  {
    (char *)"headphones", XtCBoolean, XtRBoolean, sizeof(Boolean),
    XtOffsetOf(BTResources, headphones), XtRImmediate, (XtPointer) False
  },
  {
    (char *)"sleep", XtCBoolean, XtRBoolean, sizeof(Boolean),
    XtOffsetOf(BTResources, sleep), XtRImmediate, (XtPointer) False
  },
  {
    (char *)"r_rated", XtCBoolean, XtRBoolean, sizeof(Boolean),
    XtOffsetOf(BTResources, r_rated), XtRImmediate, (XtPointer) False
  },
  {
    (char *)"a_team", XtCBoolean, XtRBoolean, sizeof(Boolean),
    XtOffsetOf(BTResources, a_team), XtRImmediate, (XtPointer) False
  },
  {
    (char *)"mute", XtCBoolean, XtRBoolean, sizeof(Boolean),
    XtOffsetOf(BTResources, mute), XtRImmediate, (XtPointer) False
  },
  {
    (char *)"no_server", XtCBoolean, XtRBoolean, sizeof(Boolean),
    XtOffsetOf(BTResources, no_server), XtRImmediate, (XtPointer) False
  },
  {
    (char *)"serverHost", XtCString, XtRString, sizeof(String),
    XtOffsetOf(BTResources, serverHost), XtRImmediate,
    (XtPointer) BT_SERVER_HOST
  },
  {
    (char *)"serverPort", XtCValue, XtRInt, sizeof(int),
    XtOffsetOf(BTResources, serverPort), XtRImmediate,
    (XtPointer) BT_SERVER_PORT
  },
  {
    (char *)"keymappings", XtCTranslations, XtRTranslationTable,
    sizeof(XtTranslations), XtOffsetOf(BTResources, keymappings), XtRImmediate,
    (XtPointer) def_translation_tab
  },
  {
    (char *)"blackColor", XtCColor, XtRPixel, sizeof(Pixel),
    XtOffsetOf(BTResources, blackColor), XtRImmediate,
    (XtPointer) 0x000000
  },
  {
    (char *)"ivoryColor", XtCColor, XtRPixel, sizeof(Pixel),
    XtOffsetOf(BTResources, ivoryColor), XtRImmediate,
    (XtPointer) 0xEEEEE0
  },
  {
    (char *)"grayColor", XtCColor, XtRPixel, sizeof(Pixel),
    XtOffsetOf(BTResources, grayColor), XtRImmediate,
    (XtPointer) 0xA8A8A8
  },
  {
    (char *)"yellowColor", XtCColor, XtRPixel, sizeof(Pixel),
    XtOffsetOf(BTResources, yellowColor), XtRImmediate,
    (XtPointer) 0xEEEE00
  },
  {
    (char *)"darkYellowColor", XtCColor, XtRPixel, sizeof(Pixel),
    XtOffsetOf(BTResources, darkYellowColor), XtRImmediate,
    (XtPointer) 0xDAA520
  },
  {
    (char *)"blueColor", XtCColor, XtRPixel, sizeof(Pixel),
    XtOffsetOf(BTResources, blueColor), XtRImmediate,
    (XtPointer) 0x0000CD
  },
  {
    (char *)"darkBlueColor", XtCColor, XtRPixel, sizeof(Pixel),
    XtOffsetOf(BTResources, darkBlueColor), XtRImmediate,
    (XtPointer) 0x00008B
  },
  {
    (char *)"neutralColor", XtCColor, XtRPixel, sizeof(Pixel),
    XtOffsetOf(BTResources, neutralColor), XtRImmediate,
    (XtPointer) 0xBFBFBF
  },
  {
    (char *)"redColor", XtCColor, XtRPixel, sizeof(Pixel),
    XtOffsetOf(BTResources, redColor), XtRImmediate,
    (XtPointer) 0xEE0000
  },
  {
    (char *)"darkRedColor", XtCColor, XtRPixel, sizeof(Pixel),
    XtOffsetOf(BTResources, darkRedColor), XtRImmediate,
    (XtPointer) 0x8B0000
  },
  {
    (char *)"orangeColor", XtCColor, XtRPixel, sizeof(Pixel),
    XtOffsetOf(BTResources, orangeColor), XtRImmediate,
    (XtPointer) 0xEE9A00
  },
  {
    (char *)"darkOrangeColor", XtCColor, XtRPixel, sizeof(Pixel),
    XtOffsetOf(BTResources, darkOrangeColor), XtRImmediate,
    (XtPointer) 0xDA7600
  },
  {
    (char *)"greenColor", XtCColor, XtRPixel, sizeof(Pixel),
    XtOffsetOf(BTResources, greenColor), XtRImmediate,
    (XtPointer) 0x32CD32
  },
  {
    (char *)"darkGreenColor", XtCColor, XtRPixel, sizeof(Pixel),
    XtOffsetOf(BTResources, darkGreenColor), XtRImmediate,
    (XtPointer) 0x228B22
  },
  {
    (char *)"cyanColor", XtCColor, XtRPixel, sizeof(Pixel),
    XtOffsetOf(BTResources, cyanColor), XtRImmediate,
    (XtPointer) 0x009ACD
  },
  {
    (char *)"darkCyanColor", XtCColor, XtRPixel, sizeof(Pixel),
    XtOffsetOf(BTResources, darkCyanColor), XtRImmediate,
    (XtPointer) 0x436EEE
  },
  {
    (char *)"purpleColor", XtCColor, XtRPixel, sizeof(Pixel),
    XtOffsetOf(BTResources, purpleColor), XtRImmediate,
    (XtPointer) 0xA020F0
  },
  {
    (char *)"darkPurpleColor", XtCColor, XtRPixel, sizeof(Pixel),
    XtOffsetOf(BTResources, darkPurpleColor), XtRImmediate,
    (XtPointer) 0x68228B
  }
};

static XrmOptionDescRec g_options[] = {
  { (char *)"-p", (char *)".headphones", XrmoptionNoArg, (caddr_t) "True" },
  { (char *)"-s", (char *)".sleep", XrmoptionNoArg, (caddr_t) "True" },
  { (char *)"-r", (char *)".r_rated", XrmoptionNoArg, (caddr_t) "True" },
  { (char *)"-a", (char *)".a_team", XrmoptionNoArg, (caddr_t) "True" },
  { (char *)"-m", (char *)".mute", XrmoptionNoArg, (caddr_t) "True" },
  { (char *)"-X", (char *)".no_server", XrmoptionNoArg, (caddr_t) "True" },
  { (char *)"-S", (char *)".serverHost", XrmoptionSepArg, (caddr_t) NULL },
  { (char *)"-P", (char *)".serverPort", XrmoptionSepArg, (caddr_t) NULL }
};

static SigReceiver g_sigrec;
static BTSigTermHandler g_termhandler;
static BTSigPipeHandler g_pipehandler;

void bt_terminate(int retval)
{
  if(g_startup)
    delete g_startup;

  if(DISPLAY)
    delete DISPLAY;

  if(g_colormap != XDefaultColormap(g_display, g_screen))
    XFreeColormap(g_display, g_colormap);

  if(g_pixmap != XmUNSPECIFIED_PIXMAP)
    XFreePixmap(g_display, g_pixmap);

  if(g_GC != XDefaultGC(g_display, g_screen))
     XFreeGC(g_display, g_GC);

  if(toplevel_)
    delete toplevel_;
  else if(g_toplevel)
    XtDestroyWidget(g_toplevel);

  if(g_display)
    XtCloseDisplay(g_display);
  if(g_appctx)
    XtDestroyApplicationContext(g_appctx);

  close(g_signalfds[0]);
  close(g_signalfds[1]);

  exit(retval);
}

static void print_usage(ostream& os)
{
  os << "Usage: BattleTris -h | -H | -V | [-s] [-p] [-m] [-S host] [-P port]\n";
  os << "                  [-display displayname] [-xrm resource-pair]\n\n";
  os << "  -h                    Display usage information.\n";
  os << "  -H                    Display licensing information.\n";
  os << "  -V                    Display version and compilation information.\n";
  os << "  -s                    Start BattleTris in sleep-mode.  This is\n";
  os << "                        equivalent to starting BattleTris and then\n";
  os << "                        selecting the sleep button after startup.\n";
  os << "  -p                    Enable headphones.  This flag indicates\n";
  os << "                        that BattleTris should play sounds through\n";
  os << "                        the headphone jack if one is available.\n";
  os << "  -m                    Mute sound.  This option overrides -p.\n";
  os << "  -S host               Specify the BattleTris Server hostname.\n";
  os << "                        Use ``BattleTris -V'' to see the default.\n";
  os << "  -P port               Specify the BattleTris Server port number.\n";
  os << "                        Use ``BattleTris -V'' to see the default.\n";
  os << "  -display displayname  Specify X server to contact.\n";
  os << "  -xrm resource-pair    Specify an application resource name/value\n";
  os << "                        pair to set in the resource database.\n";
  os << flush;
}

static void print_license(ostream& os)
{
  os << "BattleTris by Bryan Cantrill, Charlie Hoecker, and Mike Shapiro\n";
  os << "*** LICENSING INFO NOT AVAILABLE YET ***\n";
  os << flush;
}

static void print_version(ostream& os)
{
  os << "BattleTris by Bryan Cantrill, Charlie Hoecker, and Mike Shapiro\n";
  os << "BattleTris Version and Compilation Information:\n";
  os << " Version String: " << BT_VERSION << '\n';
  os << " Version Number: " << BT_MAJOR_VER << '.' << BT_MINOR_VER << '\n';
  os << "Enabled Weapons: " << BT_MAX_WEAPONS << '\n';
  os << "  Include flags: " << MKIFLAGS << '\n';
  os << "   Loader flags: " << MKLDFLAGS << '\n';
  os << " Compiler flags: " << MKCXXFLAGS << '\n';
  os << "      Libraries: " << MKLIBS << '\n';
  os << " Default server: " << BT_SERVER_HOST << '\n';
  os << "   Default port: " << BT_SERVER_PORT << '\n';
  os << flush;
}

static Boolean find_argument(int argc, char *argv[], const char *arg)
{
  while(--argc) {
    if(strcmp(argv[argc], arg) == 0)
      return True;
  }

  return False;
}

static Visual *find_24_bit_true_color(Display *display, int screen)
{
  XVisualInfo *vis_array;
  XVisualInfo vis_info;

  Visual *vis = (Visual *) 0;
  int num_vis;

  vis_info.screen = screen;
  vis_info.c_class = TrueColor;

  vis_array = XGetVisualInfo(display, VisualClassMask | VisualScreenMask,
                             &vis_info, &num_vis);

  if((vis_array == 0) || (num_vis < 1))
    return (Visual *) 0;

  int colormap_size = 0;
  int chosen = -1;

  for(int i = 0; i < num_vis; i++) {
    if(vis_array[i].colormap_size > colormap_size && vis_array[i].depth == 24){
      colormap_size = vis_array[i].colormap_size;
      vis = vis_array[i].visual;
      chosen = i;
    }
  }

  if ( chosen >= 0 ) {
    g_depth = vis_array[chosen].depth;
    g_rootWindow = RootWindow( display, g_screen );
  }
    
  XFree((caddr_t) vis_array);
  return vis;
}

static Visual *find_deepest_pseudo(Display *display, int screen, int *depth)
{
  XVisualInfo *vis_array;
  XVisualInfo vis_info;

  Visual *vis = (Visual *) 0; 
  int num_vis;

  vis_info.screen = screen;
  vis_info.c_class = PseudoColor;

  vis_array = XGetVisualInfo(display, 0, //VisualClassMask | VisualScreenMask,
                             &vis_info, &num_vis);

  if ((vis_array == NULL) || (num_vis < 1)) {
    cerr << "BattleTris: No visual info" << endl;
    return (NULL);
  }

  int colormap_size = 0;
  int chosen = -1;

  for(int i = 0; i < num_vis; i++) {
    if(vis_array[i].colormap_size > colormap_size) {
      colormap_size = vis_array[i].colormap_size;
      *depth = vis_array[i].depth;
      vis = vis_array[i].visual;
      chosen = i;
    }
  }

  if ( chosen >= 0 ) {
    g_depth = vis_array[chosen].depth;
    g_rootWindow = RootWindow( display, g_screen );
  }
    

  XFree((caddr_t) vis_array);
  return vis;
}

static int load_resources(Widget toplevel)
{
  int retval = BT_SUCCESS;

  XtGetApplicationResources(toplevel, &g_resources, g_resdefs,
                            XtNumber(g_resdefs), NULL, 0);

  if(g_resources.serverHost == (String) NULL ||
     *g_resources.serverHost == '\0') {
    cerr << "BattleTris: You must specify a valid server hostname" << endl;
    retval = BT_ERROR;
  }

  if(g_resources.serverPort < 0 || g_resources.serverPort > 65535) {
    cerr << "BattleTris: Server port must be in the range 0 - 65535" << endl;
    retval = BT_ERROR;
  }

  return retval;
}

static int x11_nonfatal(Display *display, XErrorEvent *event)
{
  static char errbuf[1024];

  XGetErrorText(display, event->error_code, errbuf, sizeof(errbuf));

  cerr << "BattleTris: X11 ERROR: " << errbuf << endl;
  cerr << "BattleTris: X11 ERROR: Serial no " << event->serial
       << ", Op code " << (int) event->request_code
       << '.' << (int) event->minor_code
       << ", Err code " << (int) event->error_code << endl;
  cerr << "BattleTris: X11 ERROR: Resource id " << event->resourceid
       << ", Display " << DisplayString(display) << endl;

  if(event->error_code == BadAlloc)
    bt_terminate(BT_ERROR);

  return BT_ERROR;
}

static int x11_fatal(Display *display)
{
  cerr << "BattleTris: Fatal X i/o error on display "
       << DisplayString(display) << endl;

  bt_terminate(BT_ERROR);
  return BT_ERROR; // Avoid compiler warning
}

static void toolkit_warning(String msg)
{
  if(msg && *msg)
    cerr << "BattleTris: Xt WARNING: " << msg << endl;
}

static void toolkit_error(String msg) __attribute__((noreturn));
static void toolkit_error(String msg)
{
  if(msg && *msg)
    cerr << "BattleTris: Xt ERROR: " << msg << endl;
  else
    cerr << "BattleTris: Xt ERROR: Unknown" << endl;

  bt_terminate(BT_ERROR);
  exit(1);
}

static void toolkit_destroy(Widget widget, XtPointer data, XtPointer cbs)
{
  g_sigrec.disable(SIGINT);
  g_sigrec.disable(SIGHUP);
  g_sigrec.disable(SIGTERM);
  g_sigrec.disable(SIGPIPE);

  bt_terminate(BT_SUCCESS);
}

static void toolkit_signal(XtPointer data, int *fd_ptr, XtInputId *id)
{
  cerr << "BattleTris: Termination signal received" << endl;
  toolkit_destroy(g_toplevel, (XtPointer) NULL, (XtPointer) NULL);
}

static int toolkit_init(int *argcptr, char *argv[])
{
  size_t argv_size = *argcptr * sizeof(char *);
  char **argv_copy;
  int argc_copy = *argcptr;
  Atom atom;

  Arg args[20];
  int i = 0;

  extern const char *BTFallbacks[];

  if((argv_copy = new char * [argc_copy]) == (char **) 0) {
    cerr << "BattleTris: Failed to allocate needed buffer" << endl;
    return BT_ERROR;
  }

  bcopy((char *) argv, (char *) argv_copy, argv_size);

  XtToolkitInitialize();
  g_appctx = XtCreateApplicationContext();

  XtAppSetFallbackResources(g_appctx, (char **)BTFallbacks);
  XtAppSetWarningHandler(g_appctx, toolkit_warning);
  XtAppSetErrorHandler(g_appctx, toolkit_error);

  g_display = XtOpenDisplay(g_appctx, NULL, NULL, "BattleTris", g_options,
                            XtNumber(g_options), ARGC_PTR argcptr, argv);

  XtAppAddActions( g_appctx, def_actions, 6 );

  if(g_display == (Display *) NULL) {
    cerr << "BattleTris: Failed to open X display" << endl;
    delete [] argv_copy;
    return BT_ERROR;
  }

  XSetErrorHandler(x11_nonfatal);
  XSetIOErrorHandler(x11_fatal);

  g_screen = DefaultScreen(g_display);
  g_visual = DefaultVisual(g_display, g_screen);
  g_rootWindow = RootWindow(g_display, g_screen);
  g_depth = DefaultDepth(g_display, g_screen);

  Visual *vis = g_visual;
  int depth = g_depth;

  g_colormap = XDefaultColormap( g_display, g_screen );
  g_GC = XDefaultGC( g_display, g_screen );

  if(vis == (Visual *) 0) {
    cerr << "BattleTris: Failed to find PseudoColor visual" << endl;
    cerr << "BattleTris: Cannot run without suitable visual" << endl;
    delete [] argv_copy;
    return BT_ERROR;
  }

  if(vis != g_visual) {
    g_visual = vis;

    g_pixmap =
      XCreatePixmap(g_display, RootWindow(g_display, g_screen), 1, 1, depth);

    g_colormap =
      XCreateColormap(g_display, RootWindow(g_display, g_screen), vis, AllocNone);

    // Create a dummy window to make a GC for the visual.
    // Straight out of Compton?  No... straight out of xv.
    Window win;
    XSetWindowAttributes xswa;
    XGCValues xgcv;
    unsigned long xswamask;
   
    XFlush(g_display);
    XSync(g_display, False);
   
    xswa.background_pixel = 0;
    xswa.border_pixel     = 1;
    xswa.colormap         = g_colormap;
    xswamask = CWBackPixel | CWBorderPixel | CWColormap;
 
    win = XCreateWindow(g_display, g_rootWindow, 0, 0, 100, 100, 2, g_depth,
                        InputOutput, vis, xswamask, &xswa);
 
    XFlush(g_display);
    XSync(g_display, False);
 
    g_GC = XCreateGC(g_display, win, 0L, &xgcv);
 
    XDestroyWindow(g_display, win);

    XtSetArg(args[i], XtNborderPixmap, g_pixmap); i++;
    XtSetArg(args[i], XtNbackgroundPixmap, g_pixmap); i++;
    XtSetArg(args[i], XtNcolormap, g_colormap); i++;
    XtSetArg(args[i], XtNdepth, depth); i++;
    XtSetArg(args[i], XtNvisual, vis); i++;
  }

  XtSetArg(args[i], XtNargc, argc_copy); i++;
  XtSetArg(args[i], XtNargv, argv_copy); i++;

  g_toplevel =
    XtAppCreateShell("BattleTris", "BTShell", applicationShellWidgetClass,
                     g_display, args, i);

  delete [] argv_copy;

  if(*argcptr > 1) {
    if(argv[1][0] == '-')
      cerr << "BattleTris: illegal option -- " << &argv[1][1] << endl;
    else
      cerr << "BattleTris: illegal argument -- " << argv[1] << endl;

    XtDestroyWidget(g_toplevel);
    XtCloseDisplay(g_display);
    XtDestroyApplicationContext(g_appctx);

    print_usage(cerr);
    return BT_USAGE;
  }

  if((atom = XmInternAtom(g_display, (char *)"WM_SAVE_YOURSELF", True)) != None)
    XmAddWMProtocolCallback(g_toplevel, atom, toolkit_destroy, (caddr_t) NULL);

  if((atom = XmInternAtom(g_display, (char *)"WM_DELETE_WINDOW", True)) != None)
    XmAddWMProtocolCallback(g_toplevel, atom, toolkit_destroy, (caddr_t) NULL);

  XtAppAddInput(g_appctx, g_signalfds[0], (XtPointer) XtInputReadMask,
                toolkit_signal, (XtPointer) NULL);

  return load_resources(g_toplevel);
}

int main(int argc, char *argv[])
{
  char procpath[1024];
  int retval;

  if(find_argument(argc, argv, "-h")) {
    print_usage(cout);
    return BT_SUCCESS;
  }

  if(find_argument(argc, argv, "-H")) {
    print_license(cout);
    return BT_SUCCESS;
  }

  if(find_argument(argc, argv, "-V")) {
    print_version(cout);
    return BT_SUCCESS;
  }

  if(pipe(g_signalfds) < 0) {
    cerr << "BattleTris: Failed to open internal pipe" << endl;
    return BT_ERROR;
  }

  if((retval = toolkit_init(&argc, argv)) != BT_SUCCESS)
    return retval;

  g_sigrec.reset();
  g_sigrec.disable(SIGTSTP);

  g_sigrec.install(SIGINT, &g_termhandler);
  g_sigrec.install(SIGHUP, &g_termhandler);
  g_sigrec.install(SIGTERM, &g_termhandler);
  g_sigrec.install(SIGPIPE, &g_pipehandler);

  toplevel_ = new BTWidget(0, g_toplevel);

  if((DISPLAY = new BTXDisplay) == 0) {
    cerr << "BattleTris: Failed to initialize X display" << endl;
    bt_terminate(BT_ERROR);
  }

  if((g_startup = new BTStartup(toplevel_)) == 0) {
    cerr << "BattleTris: Failed to initialize" << endl;
    bt_terminate(BT_ERROR);
  }

  XtRealizeWidget(g_toplevel);

  const char *legacy_fixture = getenv("BATTLETRIS_LEGACY_FIXTURE");
  if(legacy_fixture && strcmp(legacy_fixture, "game-bazaar") == 0)
    g_startup->showBazaarFixture();
  else if(legacy_fixture && strcmp(legacy_fixture, "game-recon") == 0)
    g_startup->showReconFixture();
  else if(legacy_fixture) {
    cerr << "BattleTris: unknown legacy fixture: " << legacy_fixture << endl;
    bt_terminate(BT_USAGE);
  } else if(g_resources.sleep)
    g_startup->handleSleep();
  else
    g_startup->show(1);

  XtAppMainLoop(g_appctx);
}

extern "C" {
  extern pid_t boot2(void);
  extern void boot3(const char *);
}

const char *itoa(int num, int base, int uns)
{
  static const char digits[] = "0123456789abcdef";
  static char buf[16];
 
  int i = 16, neg = (!uns && (num < 0));
  unsigned int num1;
        
  buf[--i] = 0;
  num1 = neg ? -num : num;
 
  do {
    buf[--i] = digits[num1 % base];
    num1 /= base;
  } while(num1 != 0);
 
  if(neg)
    buf[--i] = '-';
 
  return (const char *) &buf[i];
}
