/****************************************************************/
/*    NAME: Charles Hoecker                                     */
/*    ACCT: cs032100                                            */
/*    FILE: BTBazaar.C                                          */
/*    ASGN: Images                                              */
/*    DATE: Wed Apr 20 17:56:27 1994                            */
/****************************************************************/

#include "BTConfig.H"

#if HAVE_UNISTD_H
# include <unistd.h>
#endif


#include <iostream>
using namespace std;
#include <stdio.h>

#include "BTBazaar.H"
#include "BTScrolledTextWidget.H"
#include "BTScrolledListWidget.H"
#include "BTPushButtonWidget.H"
#include "BTFormWidget.H"
#include "BTRowColumnWidget.H"
#include "BTLabelWidget.H"
#include "BTPimp.H"
#include "BTArsenal.H"
#include "BTWeapon.H"
#include "BTPixmap.H"

#define BT_BAZAAR_FRAC_BASE 800

#define yfrac 1
#define xfrac 1

#define BT_BAZAAR_DRAWING_AREA_X1 20 * xfrac
#define BT_BAZAAR_DRAWING_AREA_Y1 18 * yfrac
#define BT_BAZAAR_DRAWING_AREA_WIDTH 490 * xfrac
#define BT_BAZAAR_DRAWING_AREA_HEIGHT 164 * yfrac
#define BT_BAZAAR_DRAWING_AREA_X2 510 * xfrac
#define BT_BAZAAR_DRAWING_AREA_Y2 182 * yfrac

#define BT_BAZAAR_TITLE_X1 530
#define BT_BAZAAR_TITLE_Y1 20
#define BT_BAZAAR_TITLE_X2 780
#define BT_BAZAAR_TITLE_Y2 180

#define BT_BAZAAR_WEAP_LIST_X1 20 * xfrac
#define BT_BAZAAR_WEAP_LIST_Y1 200 * yfrac
#define BT_BAZAAR_WEAP_LIST_X2 300 * xfrac
#define BT_BAZAAR_WEAP_LIST_Y2 780 * yfrac

#define BT_BAZAAR_WEAP_DESC_X1 340 * xfrac
#define BT_BAZAAR_WEAP_DESC_Y1 600 * yfrac
#define BT_BAZAAR_WEAP_DESC_X2 780 * xfrac
#define BT_BAZAAR_WEAP_DESC_Y2 780 * yfrac
#define BT_BAZAAR_WEAP_DESC_WIDTH 37

#define BT_BAZAAR_ARSENAL_X1 500 * xfrac
#define BT_BAZAAR_ARSENAL_Y1 200 * yfrac
#define BT_BAZAAR_ARSENAL_X2 780 * xfrac
#define BT_BAZAAR_ARSENAL_Y2 500 * yfrac
#define BT_BAZAAR_ARSENAL_SPACING 1 * yfrac

#define BT_BAZAAR_MSG_X1 500 * xfrac
#define BT_BAZAAR_MSG_X2 780 * xfrac
#define BT_BAZAAR_MSG_Y1 520 * yfrac
#define BT_BAZAAR_MSG_Y2 580 * yfrac

#define BT_BAZAAR_ADD_BUTTON_X1 340 * xfrac
#define BT_BAZAAR_ADD_BUTTON_Y1 365 * yfrac
#define BT_BAZAAR_ADD_BUTTON_X2 460 * xfrac
#define BT_BAZAAR_ADD_BUTTON_Y2 415 * yfrac

#define BT_BAZAAR_REMOVE_BUTTON_X1 340 * xfrac
#define BT_BAZAAR_REMOVE_BUTTON_Y1 435 * yfrac
#define BT_BAZAAR_REMOVE_BUTTON_X2 460 * xfrac
#define BT_BAZAAR_REMOVE_BUTTON_Y2 485 * yfrac

#define BT_BAZAAR_DONE_BUTTON_X1 340 * xfrac
#define BT_BAZAAR_DONE_BUTTON_Y1 505 * yfrac
#define BT_BAZAAR_DONE_BUTTON_X2 460 * xfrac
#define BT_BAZAAR_DONE_BUTTON_Y2 575 * yfrac

#define BT_BAZAAR_FUNDS_X1 325 * xfrac
#define BT_BAZAAR_FUNDS_Y1 215 * yfrac

#define BT_BAZAAR_FUNDS_WIDTH 150 * xfrac
#define BT_BAZAAR_FUNDS_HEIGHT 100 * yfrac

#define BT_BAZAAR_FUNDS_LABEL_HEIGHT 30 * yfrac

#define BT_BAZAAR_WEAPON_WIDTH 280 * xfrac
#define BT_BAZAAR_WEAPON_HEIGHT 30 * yfrac

#define BT_BAZAAR_BUTTON_WIDTH 120 * xfrac
#define BT_BAZAAR_BUTTON_HEIGHT 50 * yfrac

#define BT_WEAPON_NUM 10

#define BT_BAZ_MESSAGE_X 700
#define BT_BAZ_MESSAGE_Y 300

BTBazaar::BTBazaar( BTWidget *parent, BTPimp *pimp, BTCommManager *comm_manager, BTPixmap *image )
: parent_(parent), pimp_(pimp), initialized_(0), comm_manager_(comm_manager),
  dimmed_ (0) {
    
    form_ = new BTFormWidget( parent, "BTBazaar", BT_BAZAAR_WIDTH,
			      BT_BAZAAR_HEIGHT, BT_BAZAAR_FRAC_BASE );
    
    weap_desc_ = new BTScrolledTextWidget( form_, "weap_desc", " ");
    form_->placeChild( weap_desc_,
		       BT_BAZAAR_WEAP_DESC_X1,
		       BT_BAZAAR_WEAP_DESC_Y1,
		       BT_BAZAAR_WEAP_DESC_X2,
		       BT_BAZAAR_WEAP_DESC_Y2 );
    weap_desc_->manage();
    
    weap_list_ = new BTScrolledListWidget( form_, "weap_list", NULL, 3);
    form_->placeChild( weap_list_,
		       BT_BAZAAR_WEAP_LIST_X1,
		       BT_BAZAAR_WEAP_LIST_Y1,
		       BT_BAZAAR_WEAP_LIST_X2,
		       BT_BAZAAR_WEAP_LIST_Y2 );
    weap_list_->manage();
    
    weap_list_->addDefActionCallback( handleWeaponSelection_CB, this );
    
    weap_list_->addBrowseSelCallback( handleWeaponSelection_CB, this );
    
    add_button_ = new BTPushButtonWidget( form_, "add_button", "Add >> ");
    form_->placeChild( add_button_,
		       BT_BAZAAR_ADD_BUTTON_X1,
		       BT_BAZAAR_ADD_BUTTON_Y1,
		       BT_BAZAAR_ADD_BUTTON_X2,
		       BT_BAZAAR_ADD_BUTTON_Y2 );
    add_button_->manage();
    
    add_button_->addActivateCallback(handleAdd_CB, this);
    
    remove_button_ = new BTPushButtonWidget( form_, "remove_button",
					     "<< Remove" );
    form_->placeChild( remove_button_,
		       BT_BAZAAR_REMOVE_BUTTON_X1,
		       BT_BAZAAR_REMOVE_BUTTON_Y1,
		       BT_BAZAAR_REMOVE_BUTTON_X2,
		       BT_BAZAAR_REMOVE_BUTTON_Y2 );
    remove_button_->manage();
    
    remove_button_->addActivateCallback(handleRemove_CB, this);
    
    done_button_ = new BTPushButtonWidget( form_, "done_button", "DONE" );
    form_->placeChild( done_button_,
		       BT_BAZAAR_DONE_BUTTON_X1,
		       BT_BAZAAR_DONE_BUTTON_Y1,
		       BT_BAZAAR_DONE_BUTTON_X2,
		       BT_BAZAAR_DONE_BUTTON_Y2 );
    done_button_->manage();
    
    funds_label_ = new BTLabelWidget( form_, "funds_label",
				      "Funds" );
    
    form_->placeChild( funds_label_,
		       BT_BAZAAR_FUNDS_X1, BT_BAZAAR_FUNDS_Y1,
		       BT_BAZAAR_FUNDS_X1+BT_BAZAAR_FUNDS_WIDTH,
		       BT_BAZAAR_FUNDS_Y1+BT_BAZAAR_FUNDS_LABEL_HEIGHT );
    
    funds_label_->alignCenter();
    funds_label_->manage();
    
    funds_w_ = new BTLabelWidget( form_, "funds", "");
    
    form_->placeChild( funds_w_,
		       BT_BAZAAR_FUNDS_X1,
		       BT_BAZAAR_FUNDS_Y1+BT_BAZAAR_FUNDS_LABEL_HEIGHT,
		       BT_BAZAAR_FUNDS_WIDTH+BT_BAZAAR_FUNDS_X1,
		       BT_BAZAAR_FUNDS_HEIGHT+BT_BAZAAR_FUNDS_Y1);
    funds_w_->alignCenter();
    funds_w_->noResize();
    funds_w_->manage();
    
    label_ = new BTPushButtonWidget*[BT_WEAPON_NUM];
    
    char label[] = "arsenal ";
    
    int i, height = BT_BAZAAR_ARSENAL_Y1;
    int weap_inc = (BT_BAZAAR_ARSENAL_Y2 - BT_BAZAAR_ARSENAL_Y1) / BT_WEAPON_NUM;
    
    for (i = 0 ; i < BT_WEAPON_NUM ; i ++ ) {
      label[6] = '0' + i;
      label_[i] = new BTPushButtonWidget( form_, label,
					  label );
      form_->placeChild( label_[i], BT_BAZAAR_ARSENAL_X1,
			 height + BT_BAZAAR_ARSENAL_SPACING,
			 BT_BAZAAR_ARSENAL_X2,
			 height + weap_inc );
      label_[i]->addActivateCallback( handleArsenalSelection_CB, this );
      
      dim(i);
      label_[i]->alignLeft();
      label_[i]->manage();
      
      height += weap_inc;
    }
    
    message_ = new BTLabelWidget (form_, "baz_message", 
				  "Your opponent is waiting..." );
    form_->placeChild( message_,
		       BT_BAZAAR_MSG_X1,
		       BT_BAZAAR_MSG_Y1,
		       BT_BAZAAR_MSG_X2,
		       BT_BAZAAR_MSG_Y2);
    
    if ( image ) {
      i = 0;

      drawing_area_ = new BTDrawingAreaWidget( form_, "drawing_area",
					       image, image->width_,
					       image->height_ );
      form_->placeChild( drawing_area_,
			 BT_BAZAAR_DRAWING_AREA_X1,
			 BT_BAZAAR_DRAWING_AREA_Y1);
      
    }
    drawing_area_->manage();  
    
  }

void BTBazaar::dim( int num )
{
  XtVaSetValues( label_[num]->getWidget(),
		 XmNfillOnArm, False,
		 NULL );
  XtVaSetValues (label_[num]->getWidget(), XtVaTypedArg, XmNforeground,
		 XmRString, "gray75", 7, 0);
}

void BTBazaar::unDim( int num )
{
  XtVaSetValues( label_[num]->getWidget(),
		 XmNfillOnArm, True,
		 NULL );
  XtVaSetValues (label_[num]->getWidget(), XtVaTypedArg, XmNforeground,
		 XmRString, "blue", 7, 0);
}

void BTBazaar::dimButtons() {
  XtVaSetValues (add_button_->getWidget(), XmNfillOnArm, False,
		 XtVaTypedArg, XmNforeground, XmRString, "gray75", 7, 0);
  XtVaSetValues (remove_button_->getWidget(), XmNfillOnArm, False,
		 XtVaTypedArg, XmNforeground, XmRString, "gray75", 7, 0);
  XtVaSetValues (done_button_->getWidget(), XmNfillOnArm, False,
		 XtVaTypedArg, XmNforeground, XmRString, "gray75", 7, 0);
  dimmed_ = 1;
}

void
BTBazaar::setMessage(const char *message)
{
	message_->setLabel(message); 
}

void BTBazaar::updateFunds()
{
  char buf[32]; // More than big enough for max/min long integer

  sprintf(buf, "%ld", funds_);
  funds_w_->setLabel(buf);
}

void BTBazaar::show( long &funds, BTArsenal *arsenal, int carter)
{
  carter_ = carter;
  game_arsenal_ = arsenal;
  funds_ = funds;
  new_funds_ = &funds;
  updateFunds();

  BTWeapon *current;

  int w[BT_MAX_WEAPONS];
  int i, j;

  if ( ! initialized_ ) {
    for (i = 0; i < BT_MAX_WEAPONS; i++)
      weapons_[i] = (*pimp_)[i];
    for (i = 0; i < BT_MAX_WEAPONS; i++)
      for (j = i; j < BT_MAX_WEAPONS; j++)
	if (weapons_[i]->price_ > weapons_[j]->price_) {
	  current = weapons_[i];
	  weapons_[i] = weapons_[j];
	  weapons_[j] = current;
	}

    const char *weapons_list[BT_MAX_WEAPONS];
    for ( i = 0 ; i < BT_MAX_WEAPONS ; i++ )
      weapons_list[i] = strdup(weapons_[i]->name_ );
    
    weap_list_->setList(weapons_list, BT_MAX_WEAPONS);
    for ( i = 0 ; i < BT_MAX_WEAPONS ; i++ )
      delete [] weapons_list[i];
    initialized_ = 1;
  }
  for ( i = 0 ; i < BT_ARSENAL_SIZE ; i++ ) {
      dim(i);
      if ( game_arsenal_ && (current = (*game_arsenal_)[i]) ) {
	arsenal_[i] = new BTArsenalStruct;
	  arsenal_[i]->after = arsenal_[i]->before = game_arsenal_->
	    getQuantity(i);
	arsenal_[i]->weapon = current;
	updateLabel(i);
      }
      else {
	arsenal_[i] = NULL;
	label_[i]->setLabel("< Empty >");
      }
    }

  XmListSelectPos( weap_list_->getListWidget(), 1, 1 );

  a_selected_ = -1;
  w_selected_ = 0;

  form_->manage();

}

void BTBazaar::updateLabel( int num )
{
  static char buf[1024]; // Big enough for weapon name and quantity

  if(!arsenal_[num]) {
    strcpy(buf, "< Empty >");
  } else {
    if(arsenal_[num]->after > 1)
      sprintf(buf, "%s (%d)", (arsenal_[num]->weapon)->name_,
              arsenal_[num]->after);
    else
      strcpy(buf, (arsenal_[num]->weapon)->name_);
  }

  label_[num]->setLabel(buf);
}

void BTBazaar::hide()
{
  form_->unmanage();
  message_->unmanage();
  XtVaSetValues (add_button_->getWidget(), XmNfillOnArm, True,
		 XtVaTypedArg, XmNforeground, XmRString, "blue", 7, 0);
  XtVaSetValues (remove_button_->getWidget(), XmNfillOnArm, True,
		 XtVaTypedArg, XmNforeground, XmRString, "blue", 7, 0);
  XtVaSetValues (done_button_->getWidget(), XmNfillOnArm, True,
		 XtVaTypedArg, XmNforeground, XmRString, "blue", 7, 0);
  dimmed_ = 0;

  for ( int i = 0 ; i < BT_ARSENAL_SIZE ; i++ ) {
    if ( arsenal_[i] && ( arsenal_[i]->after-arsenal_[i]->before ) ) {
      for ( int j = arsenal_[i]->before ; j < arsenal_[i]->after ; j++ )
	game_arsenal_->buyWeapon(arsenal_[i]->weapon);
    }
    if (arsenal_[i])
      delete arsenal_[i];
  }
  *new_funds_ = funds_;
}

BTBazaar::~BTBazaar()
{
  delete weap_list_;
  delete weap_desc_;
  delete add_button_;
  delete remove_button_;
  delete done_button_;
  for ( int i = 0 ; i < BT_WEAPON_NUM ; i ++ )
    delete label_[i];
  delete label_;
  delete funds_w_;
  delete funds_label_;  
  delete drawing_area_;
  delete message_;
  delete form_;
}

void BTBazaar::handleWeaponSelection()
{
  w_selected_ = weap_list_->last_selection_;
  BTWeapon *weapon = weapons_[w_selected_];

  static char buf[2048]; // Big enough for lengthy weapon description

  sprintf(buf, "Price:    %hu\nDuration: %hu lines\n\n%s", 
          weapon->price_ * (1 + carter_), weapon->duration_,
	  weapon->description_);

  weap_desc_->setText(buf, 1, BT_BAZAAR_WEAP_DESC_WIDTH);
}

void BTBazaar::handleArsenalSelection(BTWidget *w) {

  int i = 0;

  while ( w != label_[i] ) i++;

  if (arsenal_[i]) a_selected_ = i;
}

void BTBazaar::handleAdd() {

  if (dimmed_)
    return;
  BTWeapon *weapon = weapons_[w_selected_];

  int price = weapon->price_ * (1 + carter_);
  int i;

  if ( price <= funds_ ) {
    int low = BT_ARSENAL_SIZE;
    for (i = 0 ; i < BT_ARSENAL_SIZE ; i++ ) {
      if ( ! arsenal_[i] ) {
	if (low == BT_ARSENAL_SIZE)
	   low = i;
      }
      else if (arsenal_[i]->weapon == weapon)
	break;
    }
    if ( ! (i < BT_ARSENAL_SIZE) )
      i = low;
    if ( i < BT_ARSENAL_SIZE ) {
      funds_-=price;
      updateFunds();
      unDim(i);
      if ( ! arsenal_[i] ) {
	arsenal_[i] = new BTArsenalStruct;
	arsenal_[i]->after = 1;
	arsenal_[i]->before = 0;
	arsenal_[i]->weapon = weapon;
	updateLabel(i);
      }
      else {
	arsenal_[i]->after++;
	updateLabel(i);
      }
    }
  }
  else { // not enough funds to buy weapon
  }
}

void BTBazaar::handleRemove() {

  if (dimmed_)
    return;
  if ( (a_selected_ != -1) && (arsenal_[a_selected_]) ) {
    if ( arsenal_[a_selected_]->after > arsenal_[a_selected_]->before ) {
      arsenal_[a_selected_]->after--;
      funds_ += (arsenal_[a_selected_]->weapon)->price_*(1+carter_);
      updateFunds();
      if ( arsenal_[a_selected_]->after == 0 ) {
	dim(a_selected_);
	delete arsenal_[a_selected_];
	arsenal_[a_selected_] = NULL;
	updateLabel(a_selected_);
	a_selected_ = -1;
      }
      else {
	updateLabel(a_selected_);
    	if ( arsenal_[a_selected_]->after == arsenal_[a_selected_]->before )
	  dim(a_selected_);
      }
    }
    else { // cant sell what you had before
    }
  }
}
