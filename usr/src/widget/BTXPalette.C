/****************************************************************/
/*    NAME:                                                     */
/*    ACCT: cgh                                                 */
/*    FILE: BTXPalette.C                                        */
/*    ASGN:                                                     */
/*    DATE: Thu Sep 28 13:12:42 1995                            */
/****************************************************************/


#include <iostream>
using namespace std;

#include "BattleTris.H"
#include "BTXPalette.H"
#include "BTConstants.H"

BTXPalette::BTXPalette(Colormap &cmap)
: colormap_(cmap)
{
  if(!allocPieceColors()) {
    createNew();

    if(!allocPieceColors()) {
      cerr << "BattleTris: Cannot allocate necessary colors" << endl;
      cerr << "BattleTris: Please close color-intensive apps" << endl;
      bt_terminate(1);
    }
  }
}

int BTXPalette::allocPieceColors() {
  Display *display = XtDisplay(g_toplevel);
  XColor dummy, dum1;

  color_[BT_BLACK] = g_resources.blackColor;
  if (!XAllocColor( display, colormap_, color_[BT_BLACK]))
    return 0;
  color_[BT_IVORY] = g_resources.ivoryColor;
  if (!XAllocColor( display, colormap_, color_[BT_IVORY]))
    return 0;
  color_[BT_GRAY] = g_resources.grayColor;
  if (!XAllocColor( display, colormap_, color_[BT_GRAY]))
    return 0;
  color_[BT_YELLOW] = g_resources.yellowColor;
  if (!XAllocColor( display, colormap_, color_[BT_YELLOW]))
    return 0;
  color_[BT_DYELLOW] = g_resources.darkYellowColor;
  if (!XAllocColor( display, colormap_, color_[BT_DYELLOW]))
    return 0;
  color_[BT_BLUE] = g_resources.blueColor;
  if (!XAllocColor( display, colormap_, color_[BT_BLUE]))
    return 0;
  color_[BT_DBLUE] = g_resources.darkBlueColor;
  if (!XAllocColor( display, colormap_, color_[BT_DBLUE]))
    return 0;
  color_[BT_NEUTRAL] = g_resources.neutralColor;
  if (!XAllocColor( display, colormap_, color_[BT_NEUTRAL]))
    return 0;
  color_[BT_RED] = g_resources.redColor;
  if (!XAllocColor( display, colormap_, color_[BT_RED]))
    return 0;
  color_[BT_DRED] = g_resources.darkRedColor;
  if (!XAllocColor( display, colormap_, color_[BT_DRED]))
    return 0;
  color_[BT_ORANGE] = g_resources.orangeColor;
  if (!XAllocColor( display, colormap_, color_[BT_ORANGE]))
    return 0;
  color_[BT_DORANGE] = g_resources.darkOrangeColor;
  if (!XAllocColor( display, colormap_, color_[BT_DORANGE]))
    return 0;
  color_[BT_GREEN] = g_resources.greenColor;
  if (!XAllocColor( display, colormap_, color_[BT_GREEN]))
    return 0;
  color_[BT_DGREEN] = g_resources.darkGreenColor;
  if (!XAllocColor( display, colormap_, color_[BT_DGREEN]))
    return 0;
  color_[BT_CYAN] = g_resources.cyanColor;
  if (!XAllocColor( display, colormap_, color_[BT_CYAN]))
    return 0;
  color_[BT_DCYAN] = g_resources.darkCyanColor;
  if (!XAllocColor( display, colormap_, color_[BT_DCYAN]))
    return 0;
  color_[BT_PURPLE] = g_resources.purpleColor;
  if (!XAllocColor( display, colormap_, color_[BT_PURPLE]))
    return 0;
  color_[BT_DPURPLE] = g_resources.darkPurpleColor;
  if (!XAllocColor( display, colormap_, color_[BT_DPURPLE]))
    return 0;
  return 1;
}

BTXPalette::~BTXPalette() {
}
  
void BTXPalette::createNew()
{
  cerr << "BattleTris: Creating private colormap ... " << flush;

  Display *display = XtDisplay(g_toplevel);
  Colormap new_cmap = XCopyColormapAndFree(display, colormap_);
  XtVaSetValues(g_toplevel, XmNcolormap, new_cmap, NULL);
  colormap_ = new_cmap;

  cerr << "Done." << endl << flush;
}
