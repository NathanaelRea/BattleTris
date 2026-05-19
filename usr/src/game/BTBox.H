/****************************************************************/
/*    NAME: Bryan Cantrill                                      */
/*    ACCT: bmc                                                 */
/*    FILE: BTBox.H                                             */
/*    ASSN:                                                     */
/*    DATE: Sun Feb  6 15:06:49 1994                            */
/****************************************************************/

#ifndef BTBOX_H
#define BTBOX_H

#include "BTConfig.H"

#include <X11/Xlib.h>
#include <X11/Xutil.h>


#include <iostream>
using namespace std;

#include "Block.H"
#include "BTConstants.H"

class BTBox;
class BTWidget;
class BTPixmap;

extern BTPixmap *box_maps[BT_MAX_BOXES];

class BTBoxManager {
private:
  Block<GC> gc_palette_; // BT_MAX_COLORS
  Display *display_;
  Window window_;
  Colormap colormap_;
  Screen *screen_;

public:
  BTBoxManager(BTWidget *da, BTPixmap *gimp = NULL);
  ~BTBoxManager();
  BTBox *create(int x, int y, int color);
  BTBox *dieCreate(int x, int y, int value);
  BTBox *happyCreate (int x, int y, int landed = 0);
  BTBox *createByID (int x, int y, int ID);
  BTBox *structureCreate( int x, int y );
  BTBox *createGimp(int x, int y, int value = 0);
  void dispose (BTBox *old);
};

class BTBox {
friend class BTBoxManager;

protected:
  int x_, y_;
  int id_;
  Display *display_;
  Window window_;
  int hidden_;
  BTPixmap *pixmap_;

public:
  BTBox( int id = 0 ) : id_(id), display_(0), hidden_(0) {}
  BTBox(Display *display, Window window, BTPixmap *pixmap, int id)
  : display_ (display), window_ (window), x_ (-1), y_ (-1), 
    id_ (id), hidden_(0), pixmap_(pixmap) {};
  virtual void moveTo(int x, int y, int no_erase = 0, int no_redraw = 0);
  virtual void redraw();
  virtual void erase();
  virtual int isRemoveable() { return 1; }
  virtual int value() { return 0; }
  virtual void value(int value) { }
  virtual int id() { if ( hidden_ ) return -1 ; else return id_; }
  virtual void landed() { }
  virtual void hide() { hidden_ = 1; }
  virtual ~BTBox() {};
};

class BTDieBox : public BTBox {
private:
  int value_;
public:
  BTDieBox( int value ) : BTBox( value + BT_DIE_1 - 1 ), value_(value) {}
  BTDieBox (Display *display, Window window, int value)
  : BTBox(display, window, box_maps[BT_DIE_1 + value - 1],
	  value + BT_DIE_1 - 1), value_ (value) {}
  void value (int value) { value_ = value; }
  int value () { return value_; }
  ~BTDieBox() {}
};

class BTHappyBox : public BTBox {
private:
  int landed_;

public:
  BTHappyBox(int landed) :  BTBox( BT_HAPPY + landed ), landed_(landed) {}
  BTHappyBox (Display *display, Window window, int landed = 0)
  : BTBox(display, window, box_maps[BT_HAPPY + landed], BT_HAPPY + landed), 
    landed_ (landed) {}
  int value() { if (landed_) return 0; else return BT_HAPPY_VAL; }
  void value(int value) { }
  void landed(); 
  ~BTHappyBox() {};
};

class BTInvisiBox : public BTBox {
private:
  int value_;
  int invisi_id_;
public:
  BTInvisiBox (int id, int value = 0) : invisi_id_ (id), value_ (value) { }
  void moveTo (int x, int y, int no_erase = 0, int no_redraw = 0) {}
  void redraw() {}
  void erase() {}
  int value() { return value_; }
  void value (int value) { value_ = value; }
  int id() { if ( hidden_ ) return -1; else return invisi_id_; }
  virtual ~BTInvisiBox() {};
};
    
class BTStructureBox : public BTBox {
public:
  BTStructureBox(Display *display, Window window)
  : BTBox(display, window, box_maps[BT_STRUCT], BT_STRUCT) {}
  int isRemoveable() { return 0; }
  ~BTStructureBox() {};

};

class BTInvisiStructureBox : public BTInvisiBox {
public:
  BTInvisiStructureBox () : BTInvisiBox( BT_STRUCT ) {}
  int isRemoveable() { return 0; }
  ~BTInvisiStructureBox() {}

};

class BTGimpBox : public BTBox {
public:
  BTGimpBox(int value) :  BTBox( BT_GIMP_ID ), value_(value) {}
  BTGimpBox (Display *display, Window window, int value = 0)
  : BTBox(display, window, box_maps[BT_GIMP_ID], BT_GIMP_ID), value_(value) {}
  void value (int value) { value_ = value; }
  int value() { return value_; }
  ~BTGimpBox() {};

protected:
  BTPixmap *gimp_;
  int value_;

};

#endif
