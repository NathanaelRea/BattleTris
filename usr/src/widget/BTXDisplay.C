/****************************************************************/
/*    NAME:                                                     */
/*    ACCT: cgh                                                 */
/*    FILE: BTXDisplay.C                                        */
/*    ASGN:                                                     */
/*    DATE: Thu Sep 28 11:56:34 1995                            */
/****************************************************************/

#include "BTXDisplay.H"
#include "BTConstants.H"

#include <iostream>
using namespace std;
#include "BTXPalette.H"

extern Widget g_toplevel;
extern XtAppContext g_appctx;
extern Colormap g_colormap;

BTXDisplay::BTXDisplay() : toplevel_(g_toplevel), app_(g_appctx) {
  display_ = XtDisplay(g_toplevel);
  palette_ = new BTXPalette(g_colormap);
}

BTXDisplay::~BTXDisplay() {
}

void BTXDisplay::addHandler( Widget w ) {
}

void BTXDisplay::handleEvents() {
  
  XtInputMask mask;
  
  while ( (mask = XtAppPending ( app_ )) ) {
    XtAppProcessEvent( app_, mask );
  }

}

unsigned long BTXDisplay::addTimeout( unsigned long delay,
			    void (*func)(void *, unsigned long *),
			    void *data )
{
  return XtAppAddTimeOut( app_, delay, func, data );
}

void BTXDisplay::removeTimeout( unsigned long id ) {
  XtRemoveTimeOut( id );
}

void BTXDisplay::newPalette() {
  palette_->createNew();
}

