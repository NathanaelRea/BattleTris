/****************************************************************/
/*    NAME: Bryan Cantrill                                      */
/*    ACCT: bmc                                                 */
/*    FILE: BTRecon.C                                           */
/*    ASSN:                                                     */
/*    DATE: Fri Apr 29 23:39:37 1994                            */
/****************************************************************/

#include "BTConfig.H"

#if STDC_HEADERS
# include <stdlib.h>
#endif

#include "BTLine.H"
#include "BTScore.H"
#include "BTPixmap.H"
#include "BTDebug.H"
#include "BTRecon.H"
#include "BTDisplay.H"
#include "BTBox.H"

#define BT_RECON_BIG 934
#define BT_RECON_SMALL 670
#define BT_RECON_X 665
#define BT_RECON_Y 30

void BTRecon::_reconcb_ (Widget, XtPointer thisp, XtPointer cd) {
  XmDrawingAreaCallbackStruct *call_data =
    (XmDrawingAreaCallbackStruct *) cd;
  int tet_ = 0;
  int reason = call_data->reason;
  BTRecon *instance = (BTRecon *) thisp;

  if(reason == XmCR_EXPOSE)
    instance->exposeEvent(); // call_data->event);
}

void BTRecon::_reconcb2_ (BTWidget *w, void *data) {
  ((BTRecon *)data)->exposeEvent();
}

void BTRecon::exposeEvent () {
  if (!initialized_) {
    box_manager_ = new BTBoxManager (drawing_area_,gimp_);
    initialized_ = 1;
  } else {
    for (int i = 0; i < BT_BOARD_WTH*BT_BOARD_HGT; i++) 
      if (map_[i]) 
        map_[i]->redraw();
  }
}    

void BTRecon::drawBoard (BTBoard *board) {
  int width = board->width_;
  int height = board->height_;
  
  double report_prob = 1;
  if (spy_token_ == BT_AMES)
    report_prob = .5;
  if (spy_token_ == BT_ACE)
    report_prob = .85;
  
  double k;
  int i,j,offset,doh;
  
  for (i = 0; i < height; i++)
    for (j = 0; j < width; j++) {
      offset = i * width + j;
      k = drand48();
      int id = board->rep_[offset];
      if (id && (k < report_prob)) {
	if ( map_[offset] != NULL )
	  doh = map_[offset]->id();
	if ( (map_[offset] != NULL) && (map_[offset ]->id() != id ) ) {
	  map_[offset]->erase();
	  box_manager_->dispose( map_[offset] );
	  map_[offset] = box_manager_->createByID (j, i, id);
	  map_[offset]->redraw();
	} else if ( map_[offset] == NULL ) {
	  map_[offset] = box_manager_->createByID (j, i, id);
	  map_[offset]->redraw();
	}
      } else if ( map_[offset] != NULL ) {
	map_[offset]->erase();
	box_manager_->dispose( map_[offset] );
	map_[offset] = 0;
      }
    }
  
  DISPLAY->flush();
}

long BTRecon::adjustFunds (long funds) {
  if (!spy_on_) 
    return funds;
  int mult = 1; 
  if (rand() % 2) 
    mult = -1;

  switch (spy_token_) {
  case BT_AMES: 
    // Funds can\'t be equal to -1 or FPE!
    if (funds == -1) funds = -2;
    return (funds + (mult * (rand() % (funds + 1))));
  case BT_ACE:
    if (tet_) {
      tet_ = 0;
      return (funds + (mult * (rand() % 100)));
    }
    return (funds);
  case BT_CONDOR:
    return (funds);
  }       
}


BTRecon::BTRecon (BTWidget *toplevel, BTWidget *form, BTPixmap *gimp) 
: BTRingNode (), toplevel_ (toplevel), spy_on_ (0),
old_lines_ (0) {

  if ( gimp ) {
    gimp_ = gimp;
    gimp->ref();
  }
  else
    gimp_ = NULL;

  for (int i = 0; i < BT_BOARD_WTH*BT_BOARD_HGT; i++)
    map_[i] = 0;

  drawing_area_ = new BTDrawingAreaWidget (form, "recon_area", 0, 
            BT_BOX_WTH * BT_BOARD_WTH,
            BT_BOX_HGT * BT_BOARD_HGT,
            BT_RECON_X, BT_RECON_Y);

  drawing_area_->attachLeftNone();
  drawing_area_->attachTopNone();
  drawing_area_->attachRightNone();
  drawing_area_->attachBottomNone();

  drawing_area_->size( BT_RECON_X, BT_RECON_Y );

  XtVaSetValues (drawing_area_->getWidget(), XtVaTypedArg, XmNbackground, XmRString, "black", 6, 0);

  drawing_area_->addExposeCallback( _reconcb2_, this );
//  XtAddCallback (rep_, XmNexposeCallback, _reconcb_, this);
  initialized_ = 0;
}

BTRecon::~BTRecon() {
  if (gimp_)
    if (gimp_->deref())
      delete gimp_;
  if (initialized_)
    delete box_manager_;
  if (drawing_area_)
    delete drawing_area_;
}

void BTRecon::receive (BTRingPacket *packet) {
  switch (packet->token) {
  case BT_START:
    spy_on_ = 0;
    old_lines_ = 0;
    break;

  case BT_WPN_LAUNCH: {
    BTWeapon *wpn = (BTWeapon *) packet->data; 
    switch (wpn->token()) {
    case BT_AMES:
    case BT_ACE:
    case BT_CONDOR: {
      spy_on_ += wpn->duration();
      spy_token_ = wpn->token();
      BTDebug ("spy_on has been set to " << spy_on_);
      toplevel_->size( -1, -1, BT_RECON_BIG );
      drawing_area_->manage();
    }
    }
    break;
  }
  case BT_CONDOR_OFF: {
    spy_on_ = 0;
    toplevel_->size( -1, -1, BT_RECON_SMALL );
    for (int i = 0; i < BT_BOARD_WTH*BT_BOARD_HGT; i++) {
      if (map_[i]) {
	map_[i]->erase();
	box_manager_->dispose (map_[i]);
      }
      map_[i] = 0;
    }
    drawing_area_->unmanage();
    break;
  }
  case BT_OP_SCORE: {
    BTScore *op_score = (BTScore *) packet->data;
    int inc = op_score->lines_ - old_lines_;
    if (spy_on_ && inc) {
      if (inc == 4) 
        tet_ = 1;
      spy_on_ = spy_on_ - inc < 0 ? 0 : spy_on_ - inc;
      if (!spy_on_)
	sendPlusMe(BT_CONDOR_OFF, 0);
    }
    old_lines_ = op_score->lines_;
    break;
  }

  case BT_BOARD : {
    if ( initialized_ ) {
      BTBoard *board = (BTBoard *) packet->data;
      switch (board->motivation()) {
      case BT_AMES:
      case BT_ACE:
      case BT_CONDOR: 
        drawBoard (board);
        break;
      }
      break;
    }
  }
  }

  pass (packet);
}
