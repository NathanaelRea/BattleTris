#ifndef BTCOMPUTER_H
#define BTCOMPUTER_H

#include "BTConfig.H"


#include <iostream>
using namespace std;

#include "Block.H"
#include "List.H"

#include "BTConstants.H"
#include "BTProtocol.H"
#include "BTRingNode.H"
#include "BTCBoard.H"
#include "BTStack.H"
#include "BTTimeOut.H"

#if STDC_HEADERS
# include <stdlib.h>
#endif

#define BT_ERNIE_NAME_LEN 256

enum BT_MOVE_DIR {
  BT_MOVE_NONE,
  BT_MOVE_LEFT,
  BT_MOVE_RIGHT,
  BT_MOVE_DOWN,
  BT_MOVE_ROTATE
};

struct BTMove {
  BTMove( int x, int y, BT_MOVE_DIR dir ) : x_(x), y_(y), dir_(dir) {}
  BTMove( BTMove &other ) : x_(other.x_), y_(other.y_), dir_(other.dir_) {}
  BTMove &operator=(BTMove &other) {
    x_ = other.x_;
    y_ = other.y_;
    dir_ = other.dir_;
    return *this;
  }
  int x_, y_;
  BT_MOVE_DIR dir_;
};

struct BTComputerMoveStruct {
	int x,y; // where to put piece
	int orientation; // how many times to rotate piece
	int target_x;
	BTStack<BTMove> path;
#ifndef SWEET
	int num_holes;
	int open_holes;
	int closed_holes;
	int lines_cleared;
	float variance,cov_hole_pen,hole_pen,height_pen;
#endif
};

enum { BTC_LAUNCH, BTC_COMBO, BTC_LAWYERS_OFF, BTC_CAN_PURCHASE };

struct BTCOrders {
  BTCOrders(unsigned long type = BTC_LAUNCH, int line_no = -1, int my_line_no = -1, int bazaar_no = -1, int weapon = -1 ) : type_(type), bazaar_no_(bazaar_no), line_no_(line_no), my_line_no_(my_line_no), weapon_(weapon) {}
  int bazaar_no_;
  int line_no_;
  int my_line_no_;
  int weapon_;
  unsigned long type_;
};

#define ORDER_MASK 0xF
#define CREATE_ORDER(A,B) ((B<<4) | A)

class BTPimp;
class BTGame;
class BTWeaponManager;
class BTPieceManager;
class BTBoardManager;
class BTCommManager;
class BTScoreManager;
class BTStartup;

class BTComputer : public BTRingNode {
private:
  BTComputerMoveStruct move_;
  unsigned long id_;

  BTPimp *pimp_;
  BTGame *btgame_;
  BTBoardManager *board_manager_;
  BTPieceManager *piece_manager_;
  BTScoreManager *score_manager_;
  BTWeaponManager *weapon_manager_;
  BTCommManager *comm_manager_;
  BTStartup *startup_;
  BTPiece *piece_;
  BTArsenal *arsenal_;
  BTCBoard cboard_, cboard_bak_;
  BTPosition piece_positions_, piece_positions_bak_;
  BTCPenalties penalties_;
  BTStack<BTMove> path_;

  float min_;

  List<BTCOrders *> commando_;
  short lines_removed_;
  
  unsigned short lawyers_,  // does the other guy have lawyers delite launched against him?
  next_weapon_,             // our next selection
  next_launch_,             // the other guy\'s line number for our next launch
  delay_;       // delay in ms between moves

  unsigned long piece_no_, moves_checked_;

  int can_purchase_[BT_MAX_WEAPONS];
  int can_launch_[BT_MAX_WEAPONS];

  int upsidedown_,
  no_happy_,
  combo_cost_,
  def_x_, 
  def_y_,
  swapper_,
  delta_y_,
  left_x_, right_x_,
  variance_penalty_, height_penalty_,
  closed_hole_penalty_, open_hole_penalty_, covered_hole_penalty_,
  line_bonus_, bit_bonus_,
  old_open_holes_, old_closed_holes_,
  piece_top_, piece_bottom_, piece_left_, piece_right_, // piece dimensions
  scan_bottom_, // line below which we don\'t care about holes in the board
  bazaar_no_,   // the bazaar number
  carter_,      // is Carter taxing our ass?
  no_nice_day_, // keeps track of number of smileys opponent has given us
  weapons_,     // number of weapons we got
  game_,        // do we have a game
  paused_,      // is the game paused
  bazaar_,      // are we in the bazaar
  left_bazaar_, op_left_bazaar_,  // bazaar-exit synchronization
  condor_,      // is the condor going
  weapons_bought_,  // # of weapons we\'ve bought
  lowline_,     // have we crossed the "safe" line?
  reloaded_,    // has a lazy susan reloaded our aresenal?
  deciding_,    // are we deciding
  make_move_,   // keeps track of how many moves Ernie should make
  update_freq_, // number of pieces ernie uses before a condor update (used b/c super ernie)
  super_,       // are we super Ernie
  rescan_,
  old_lines_;   // opponent's line count, for Lawyers' Delite

  char name_[BT_ERNIE_NAME_LEN];

  void decide();
  void checkMove( int, int, int, BT_MOVE_DIR );
  float computeValue( int, int, int );
  void copyList( List<BTMove *> &, List<BTMove *> &);
  void killList( List<BTMove *> &);
  int doMove( int &, int &, BTMove * );
  void goShopping( long & );
  void launchWeapon( int weapon_id = BT_NO_WPN );
  void reload();
  BTCOrders *activateCommando();
  void scanPiece();
  void clearCommando();
  int priceWeapon( int weapon );
  int purchaseApproved();
  BTCOrders *findOrder( unsigned long, int = BT_NO_WPN, int remove = 0 );
  void handleEvents();
  void checkBazaar();
  BT_TIMEOUT_CALLBACK(BTComputer, leaveBazaar);

public:
  BTComputer ( BTCommManager *, BTGame *, BTPimp * );
  void receive(BTRingPacket *packet);
  int run();
  void reset( int = 0 );
  ~BTComputer();
  static void _cmptimeout_( void *, unsigned long * );
  int deciding() { return deciding_; }
  void removeTimer();
  const char *name() { return name_; }
	static int nLevels();
	static const char *levelName(int level);
};

#endif
