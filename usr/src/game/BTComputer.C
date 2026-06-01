#include "BTConfig.H"

#if STDC_HEADERS
# include <stdlib.h>
#endif

#include <assert.h>
#include <stdio.h>
#include <limits.h>
#include <math.h>

#ifndef __sun
#include <float.h>
#endif

#include "ListIter.H"

#include "BTPieceManager.H"
#include "BTScoreManager.H"
#include "BTWeaponManager.H"
#include "BTBoardManager.H"
#include "BTCommManager.H"
#include "BTStartup.H"
#include "BTPimp.H"
#include "BTGame.H"
#include "BTComputer.H"
#include "BTXDisplay.H"
#include "BTBox.H"

//#define COMP_DEBUG 1
#ifdef COMP_DEBUG
#define WEAP_DEBUG 1
#define BTDebug2(x)  cerr << BT_VERSION << " [" << __FILE__ << ", " << __LINE__ << "] DEBUG: " << x << endl
#else
#define BTDebug2(x)
#endif

/* Midline = point where we`re in trouble */
#define BT_MIDLINE 14
#define BT_HIGHLINE 12 // BT_BOARD_HGT
#define BT_LOWLINE 20 // BT_BOARD_HGT
#define BT_SWAPLINE 5
#define BT_SCAN_DEPTH 4

#define BT_OPEN_HOLE_PENALTY 7000 // 5000
#define BT_CLOSED_HOLE_PENALTY 10000 // 8000
#define BT_MARGIN_PENALTY 25
#define BT_HEIGHT_PENALTY 30000
#define BT_LPIECE_PENALTY 0
#define BT_COVERED_HOLE_PENALTY 3000
#define BT_LINE_BONUS 5000
#define BT_HAPPY_BONUS 20000
#define BT_BIT_BONUS 200
#define BT_VARIANCE_PENALTY 50 // was 100
#define BT_DROP_TIME2 400000
#define BT_FALLOUT_X1 2
#define BT_FALLOUT_X2 BT_BOARD_WTH-2
#define BT_COMPUTER_INTERVAL 750
#define BT_BAZAAR_TIMEOUT 3000
#define BT_COVERED_COLUMN 5000
#define BT_CREVICE_DEPTH 4
#define BT_SMALL_CREVICE 4

#define BTC_XMOVE_DELAY 50

#define BT_SUPER_CONDOR_FREQ 1  // update condor every four pieces when super ernie enabled

#define BT_MIN_WPN_COST 1
#define BT_MIN_COMBO_COST 750
#define BT_MAX_COMBO_COST 1250

#define BT_FW_OHP 1500
#define BT_FW_CHP 3000
#define BT_FW_CVHP 50
#define BT_FW_HP 15000
#define BT_FW_LB 20000
#define BT_FW_BB 500
#define BT_FW_VP 10

#define BT_LAUNCH_DELAY 2
#define BT_REAGAN_DELAY 1

static struct {
	int timeout;
	const char *name;
} levels[] = {
	{ 4000, "Comatose" },
	{ 3000, "Languid" },
	{ 2000, "Lethargic" },
	{ 1500, "Alert" },
	{ 1250, "Able" },
	{ 1000, "Willing" },
	{ 750, "Lively" },
	{ 500, "Energetic" },
	{ 300, "Caffeinated" },
	{ 100, "Supercharged" },
	{ 10, "Hell-Bent" },
	{ 0, "Bionic" }
};

BTComputer::BTComputer( BTCommManager *comm_manager, BTGame *game,
			BTPimp *pimp )
: BTRingNode(), comm_manager_( comm_manager ), pimp_(pimp), btgame_(game)
{
  weapon_manager_  = new BTWeaponManager( NULL, 0, 0, comm_manager, 1 );
  board_manager_ = new BTBoardManager (weapon_manager_,BT_BOARD_WTH,
				       BT_BOARD_HGT,1);
  score_manager_ = new BTScoreManager (weapon_manager_,0,NULL,0,0);
  piece_manager_ = new BTPieceManager (NULL, board_manager_, NULL);
  
  // Establish token ring
  comm_manager_->next(this);
  next_ = weapon_manager_;
  weapon_manager_ -> next(board_manager_);
  board_manager_->next (piece_manager_);
  piece_manager_->next (score_manager_);
  score_manager_->next (comm_manager_);
  
  reset();
}

BTComputer::~BTComputer()
{
  // Delete all of the orders
  BTCOrders *temp;
  while (commando_.remove_head(temp))
    delete temp;

  if (piece_)
    piece_->reset();
  delete board_manager_;
  delete piece_manager_;
  delete weapon_manager_;
  delete score_manager_;
}

const char *
BTComputer::levelName(int level)
{
	return (levels[level].name);
}

int
BTComputer::nLevels()
{
	int i;

	for (i = 0; levels[i].timeout; i++)
		continue;

	return (i + 1);
}

void BTComputer::removeTimer() {
  if ( id_ ) {
    DISPLAY->removeTimeout(id_);
    id_ = 0;
  }
}

int BTComputer::priceWeapon( int weapon )
{
  return (*pimp_)[weapon]->price_*(1+carter_);
}

void BTComputer::reset(int level)
{
  super_ = levels[level].timeout == 0 ? 1 : 0;
  piece_ = 0;

	strcpy(name_, levels[level].name);
	strcat(name_, " Ernie");

  // set the weapons that can be purchased and launched
  for ( int j = 0 ; j < BT_MAX_WEAPONS ; j++ ) {
    can_purchase_[j] = 1;
    can_launch_[j] = 1;
  }

  can_purchase_[BT_ACE] =
    can_purchase_[BT_CONDOR] =
    can_purchase_[BT_AMES] =
    can_purchase_[BT_MEADOW] =
    can_purchase_[BT_SUSAN] =
    can_purchase_[BT_REAGAN] = 0;

  // Debrief the commando
  BTCOrders *orders;
  RWListIter<BTCOrders *> iter(commando_);

  for(; commando_.remove_head(orders); )
    delete orders;

  // Make an order to enable the lazy susan after the opponent has
  // gotten 40 lines.
  orders = new BTCOrders(CREATE_ORDER(BTC_CAN_PURCHASE,BT_SUSAN), 50);
  commando_.insert_after_tail( orders );

  // set the penalties for various board characteristics
  penalties_.vp_ = BT_VARIANCE_PENALTY;
  penalties_.hp_ = BT_HEIGHT_PENALTY;
  penalties_.chp_ = BT_CLOSED_HOLE_PENALTY;
  penalties_.ohp_ = BT_OPEN_HOLE_PENALTY;
  penalties_.lb_ = BT_LINE_BONUS;
  penalties_.cvhp_ = BT_COVERED_HOLE_PENALTY;
  penalties_.hap_ = BT_HAPPY_BONUS;
  penalties_.midline_ = BT_MIDLINE;

  // initialize various data members
  old_open_holes_ = old_closed_holes_ = rescan_ = scan_bottom_ = 0;
  game_ = 1;
  combo_cost_ = -1;
  upsidedown_ = no_happy_ = bazaar_no_ = 0;
  carter_ = swapper_ = reloaded_ = 0;
  make_move_ = deciding_ = no_nice_day_ = next_launch_ = bazaar_ = 0;
  lawyers_ = weapons_ = paused_ = condor_ = weapons_bought_ = 0;
  piece_no_ = 0;
  next_weapon_ = BT_MONDALE;
  arsenal_ = 0;


	delay_ = levels[level].timeout;

	if (delay_ <= 100) {
    		update_freq_ = BT_SUPER_CONDOR_FREQ;
	} else {
		update_freq_ = 1;
	}

  comm_manager_->clear();
}

void BTComputer::_cmptimeout_( void *data, unsigned long * )
{
  BTComputer *t = (BTComputer *)data;
  
  t->run();
}

void BTComputer::_bazwait_( void *data, unsigned long * )
{
  ((BTComputer *)data)->send ( BT_END_BAZ, 0 );
}

void BTComputer::receive (BTRingPacket *packet) {

  switch (packet->token) {
  case BT_START: {
    board_manager_->clear();
    id_ = DISPLAY->addTimeout(delay_, _cmptimeout_, this);
    break;
  }
    
  case BT_ERR: case BT_GAME_OVER: case BT_DEAD: {
    removeTimer();
    game_ = 0;
    board_manager_->clear();
    break;
  }
    
  case BT_LINE: {
    lines_removed_ = ((BTLine *) packet->data)->inc();
    break;
  }
    
  case BT_START_BAZ: {
    removeTimer();
    BTDebug2("Received start baz token!");
    bazaar_ = 1;
    bazaar_no_++;
    arsenal_ = weapon_manager_->arsenal_;
    goShopping( score_manager_->rep_.funds_ );
    break;
  }
  case BT_END_BAZ: {
    bazaar_ = 0;
    id_ = DISPLAY->addTimeout(delay_, _cmptimeout_, this);
    break;
  }
    
  case BT_PAUSE: {
    if ( paused_ ) {
      paused_ = 0;
      id_ = DISPLAY->addTimeout(delay_, _cmptimeout_, this);
    }
    else {
      paused_ = 1;
      removeTimer();
    }
    break;
  }
    
  case BT_BOARD:
    { 
      BTBoard *board = (BTBoard *) packet->data;
      BTDebug ("Board received token, motivation is "
		<< board->motivation());
      if ( board->motivation() == BT_SWAP ) {
	
	if (weapon_manager_->BTActive[BT_BOTTLE]) {
	  weapon_manager_->remaining_[BT_BOTTLE] = 0;
	  send (BT_WPN_OFF, (*pimp_)[BT_BOTTLE]);
	}
	if (weapon_manager_->BTActive[BT_UPBYSIDE]) {
	  weapon_manager_->remaining_[BT_UPBYSIDE] = 0;
	  sendPlusMe (BT_WPN_OFF, (*pimp_)[BT_UPBYSIDE]);
	}
	BTBoard temp (board_manager_);
	if ( !swapper_ )
	  {
	    BTDebug ("Computer did not initiate the swap -- must respond.");
	    temp.motivation(BT_SWAP);
	    comm_manager_->sendBoard (&temp);
	  }
	board_manager_->newBoard (board);
	BTDebug ("Computer: Swapped in new board.");
	swapper_ = 0;
	rescan_ = 1;
      }
      break;
    }
    
  case BT_WPN_ON: {
    BTWeapon *wpn = (BTWeapon *) packet->data; 
    switch (wpn->token()) {
      
    case BT_KEATING: {
      break;
    }
    case BT_CARTER: {
      carter_ = 1;
      break;
    }
    case BT_CONDOR: {
      condor_ = 1;
      break;
    }
    case BT_FORCE: {
      if (weapon_manager_->BTActive[wpn->token()]) break;
      penalties_.vp_ *= 2;
      penalties_.hp_ *= 4;
      penalties_.lb_ *= 4;
      break;
    }
    case BT_FOUR_BY_FOUR: {
      if (weapon_manager_->BTActive[wpn->token()]) break;
      penalties_.hp_ *= 2;
      penalties_.vp_ *= 2;
      break;
    }
    case BT_BOTTLE: {
      if (weapon_manager_->BTActive[wpn->token()]) break;
      penalties_.ohp_ /= 10;
      penalties_.chp_ /= 10;
      penalties_.hp_ *= 2;
//      penalties_.vp_ /= 15;
      rescan_ = 1;
      break;
    }
    case BT_NO_DICE: {
      if (weapon_manager_->BTActive[wpn->token()]) break;
      penalties_.vp_ *= 4;
      break;
    }
    case BT_FALL_OUT: {
      if (weapon_manager_->BTActive[wpn->token()]) break;
      penalties_.lb_ *= 2;
      penalties_.hp_ *= 2;
      rescan_ = 1;
      break;
    }
    case BT_MISSING: case BT_BLIND: case BT_BUG: case BT_PIECE_IT: {
      rescan_ = 1;
      break;
    }
    case BT_NICE_DAY: {
      no_nice_day_++;
      break;
    }
    case BT_RISE_UP: {
      lines_removed_--;
      break;
    }
    case BT_SUSAN: {
      break;
    }
    case BT_UPBYSIDE: {
      upsidedown_ = 1;
      break;
    }
    default:
      break;
    }
    break;
  }
  case BT_WPN_LAUNCH: {
    
    BTWeapon *wpn = (BTWeapon *) packet->data; 
    switch (wpn->token()) {
    case BT_LAWYERS: {
      BTCOrders *orders = findOrder (BTC_LAWYERS_OFF);
      if ( ! orders )  {
	orders = new BTCOrders(BTC_LAWYERS_OFF,
			       score_manager_->rep_.op_lines_ +
			       (*pimp_)[BT_LAWYERS]->duration());
	commando_.insert_after_tail( orders );
      }
      else
	orders->line_no_ += (*pimp_)[BT_LAWYERS]->duration();
      lawyers_ = 1;
      break;
    }
    case BT_REAGAN: {
      // If we launch a Reagan, we don\'t want to launch another economic
      // weapon for a while
      BTCOrders *orders = findOrder (CREATE_ORDER(BTC_CAN_PURCHASE,BT_REAGAN), 1);
      if ( orders )
	delete orders;
      can_purchase_[BT_REAGAN] = 0;
      can_purchase_[BT_KEATING] = 0;
      can_purchase_[BT_NICE_DAY] = 0;
      can_launch_[BT_NICE_DAY] = 0;
      orders = new BTCOrders(CREATE_ORDER(BTC_CAN_PURCHASE,BT_REAGAN),
			     score_manager_->rep_.op_lines_ + 50);
      commando_.insert_after_tail( orders );

      orders = new BTCOrders(CREATE_ORDER(BTC_CAN_PURCHASE,BT_KEATING),
			     score_manager_->rep_.op_lines_ + 50);
      commando_.insert_after_tail( orders );

      orders = new BTCOrders(CREATE_ORDER(BTC_CAN_PURCHASE,BT_NICE_DAY),
			     score_manager_->rep_.op_lines_ + 50);
      commando_.insert_after_tail( orders );
    }
    case BT_KEATING:
      can_launch_[BT_NICE_DAY] = 0;
      break;
    default:
      break;
    }
    break;
  }   
  case BT_WPN_OFF: {
    BTWeapon *wpn = (BTWeapon *) packet->data; 
    switch (wpn->token()) {
    case BT_FALL_OUT: {
      penalties_.lb_ /= 2;
      penalties_.hp_ /= 2;
      break;
    }
    case BT_UPBYSIDE: {
      upsidedown_ = 0;
      break;
    }
    case BT_CARTER: {
      carter_ = 0;
      break;
    }
    case BT_BOTTLE: {
      penalties_.ohp_ *= 10;
      penalties_.chp_ *= 10;
      penalties_.hp_ /= 2;
//      penalties_.vp_ *= 15;
      rescan_ = 1;
      break;
    }
    case BT_FOUR_BY_FOUR: {
      penalties_.lb_ /= 2;
      penalties_.hp_ /= 2;
      break;
    }
    case BT_FORCE: {
      penalties_.vp_ /= 2;
      penalties_.hp_ /= 4;
      penalties_.lb_ /= 4;
      break;
    }
    case BT_NO_DICE: {
      penalties_.vp_ /= 4;
      break;
    }
    default: {
      break;
    }
    }
    break;
  }
  case BT_CONDOR_OFF: {
    condor_ = 0;
    break;
  }
  default: {
    break;
  }
  }
  pass (packet);
}

int BTComputer::purchaseApproved()
{
  
  if ( !can_purchase_[next_weapon_] )
    return 0;

  BTCOrders *orders;
  ListIter<BTCOrders *> iter(commando_);
  for (iter.jump_before_head(); iter.peek_next( orders ); iter.inc()) {
    switch ( orders->weapon_ ) {
    case BT_CARTER:
    case BT_SUSAN:
    case BT_SWAP:
      if ( orders->weapon_ == next_weapon_ )
	return 0;
      break;
    }
  }
  return 1;
}

void BTComputer::goShopping( long &funds )
{

  if ( cboard_.top_ > BT_SWAPLINE )
    can_purchase_[BT_SWAP] = 0;
  else
    can_purchase_[BT_SWAP] = 1;

  unsigned short price = 0, old_weapons;
  
  if ( combo_cost_ == 0 ) {
    no_happy_ = 0;
    next_launch_ = score_manager_->rep_.op_lines_;
  }
  
  do {
    old_weapons = weapons_bought_;
    BTWeapon *weapon;
    
    if ( next_weapon_ == BT_NO_WPN )
      do {
	next_weapon_ = (rand() % BT_MAX_WEAPONS);
      } while ( ! purchaseApproved() );
    
    price = priceWeapon( next_weapon_ );
    
    while ( (price <= funds) && (next_weapon_ != BT_NO_WPN ) ) {
      weapon = (*pimp_)[next_weapon_];
      price = priceWeapon( next_weapon_ );
      if ( (price <= funds) && arsenal_->buyWeapon( weapon ) ) {
#ifdef WEAP_DEBUG
	cout << "Bought a " << weapon->name_ << ".\n" << endl;
#endif
	weapons_++;
	funds -= price;
	if ( combo_cost_ == -1 )
	  combo_cost_ = price;
	else
	  combo_cost_ += price;

	// Prep weapon launch
	BTCOrders *order = new BTCOrders;
	order->weapon_ = next_weapon_;
	switch( next_weapon_ ) {
	case BT_SO_LONG:
	case BT_MONDALE:
        case BT_CARTER:
	  // launch these weapons now
	  order->type_ = BTC_LAUNCH;
	  order->line_no_ = next_launch_;
	  break;
	case BT_REAGAN:
	  // now you can buy a nice day
	  can_launch_[BT_NICE_DAY] = 1;
	default:
	  // wait until we\'ve got a combination
	  order->type_ = BTC_COMBO;
	  order->line_no_ = next_launch_;
	  break;
	}
	commando_.insert_after_tail( order );


	int cur_weapon = next_weapon_;

	next_weapon_ = BT_NO_WPN;

	// determine the next weapon
	switch( cur_weapon ) {
	case BT_NICE_DAY:
	  can_launch_[BT_NICE_DAY] = 0;
	  can_purchase_[BT_NICE_DAY] = 0;
	  can_purchase_[BT_REAGAN] = 1;
	  next_weapon_ = BT_REAGAN;

	  // Launch after opponent has gotten a line after the happy
	  next_launch_ ++;
	  no_happy_ = 1;

	  // Assure that Reagan is bought before we launch the Happy
	  if ( combo_cost_ >= BT_MAX_COMBO_COST )
	    combo_cost_ = BT_MAX_COMBO_COST - 1;
	  break;
	case BT_SPEEDY:
	  if ( combo_cost_ < BT_MAX_COMBO_COST)
	    next_weapon_ = BT_SPEEDY;
	  else
	    next_weapon_ = BT_NO_WPN;
	  break;
	case BT_REAGAN:
	  can_purchase_[BT_REAGAN] = 0;
	  break;
	default:
	  ;
	}
	
	weapons_bought_++;
	if ( ((combo_cost_ >= BT_MIN_COMBO_COST) &&
	      (next_weapon_ == BT_NO_WPN)) ||
	     (cur_weapon == BT_SUSAN) )
	  combo_cost_ = 0;
      }
    }
  } while ( (weapons_bought_ != old_weapons) && combo_cost_ );

  // check to see if we\'ve just bought a combo and, if so,
  // let\'s set those weapons to launch
  if ( !combo_cost_ ) {
    ListIter<BTCOrders *> iter(commando_);
    BTCOrders *orders;

    for(iter.jump_before_head(); iter.peek_next(orders); iter.inc()) {
      if ( orders->type_ == BTC_COMBO ) {
	if ( can_launch_[orders->weapon_] ) {
	  orders->type_ = BTC_LAUNCH;
#ifdef WEAP_DEBUG
	  cout << "Combo->launch for " << orders->type_ << endl;
#endif
	}
	else {
#ifdef WEAP_DEBUG
	  cout << "Combo->launch NOT for " << orders->type_ << endl;
#endif
	}
      }
    }
  }
  DISPLAY->addTimeout( BT_BAZAAR_TIMEOUT, _bazwait_, this);
}

void BTComputer::launchWeapon( int weapon_id ) {
  
  if ( weapons_ ) {
    for ( int i = 0 ; i < BT_ARSENAL_SIZE ; i ++ ) {
      BTWeapon *weapon = (*arsenal_)[i];
      if ( weapon && ( weapon_id != BT_NO_WPN ) &&
	   ( weapon->token() == weapon_id )) {
	arsenal_->useWeapon(i);
	sendPlusMe( BT_WPN_LAUNCH, weapon );
	weapons_--;
      }
    }
  }
}

BTCOrders *BTComputer::findOrder( unsigned long type, int weapon, int remove ) {

  RWListIter<BTCOrders *> iter(commando_);
  BTCOrders *orders;

  for(iter.jump_before_head(); iter.peek_next(orders); iter.inc()) {
    if ( orders->type_ == type ) {
      if ( weapon != BT_NO_WPN ) {
	if ( weapon == orders->weapon_ ) {
	  if (remove)
	    iter.remove_next(orders);
	  return orders;
	}
      }
      else {
	if (remove)
	  iter.remove_next(orders);
	return orders;
      }
    }
  }
  return NULL;
}

void BTComputer::reload()
{
  weapons_ = 0;
  next_launch_ = score_manager_->rep_.op_lines_;
  
  // delete current orders using our arsenal
  BTCOrders *orders;
  RWListIter<BTCOrders *> iter(commando_);

  for(iter.jump_before_head(); iter.peek_next(orders); iter.inc()) {
    switch ( (orders->type_ & ORDER_MASK) ) {
    case BTC_LAUNCH:
    case BTC_COMBO:
      iter.remove_next(orders);
      delete orders;
      break;
    default:
      break;
    }
  }

  // can\'t launch NICE DAY if we don\'t have a keating...
  can_launch_[BT_NICE_DAY] = 0;

  // Get the new arsenal
  arsenal_ = weapon_manager_->arsenal_;
  combo_cost_ = 0;
  next_weapon_ = BT_NO_WPN;
  if (arsenal_ == 0)
    return;

  // reorder launches
  for ( int i = 0 ; i < BT_ARSENAL_SIZE ; i ++ ) {
    BTWeapon *weapon = (*arsenal_)[i];
    if (weapon && arsenal_->getQuantity(i) > 0 &&
	( weapon->token() != BT_NO_WPN )) {
      int quantity = arsenal_->getQuantity(i);
      for ( int j = 0 ; j < quantity ; j++ ) {
	orders = new BTCOrders;
	orders->weapon_ = weapon->token();
	orders->line_no_ = next_launch_;
	switch( orders->weapon_ ) {
	case BT_NICE_DAY:
	  if ( can_launch_[BT_NICE_DAY] )
	    orders->type_ = BTC_LAUNCH;
	  else {
	  orders->type_ = BTC_COMBO;
	  next_weapon_ = BT_KEATING;
	}
	  break;
	case BT_REAGAN:
	case BT_KEATING:
	  can_launch_[BT_NICE_DAY] = 1;
#ifdef WEAP_DEBUG
	  cout << "Can launch a nice day " << endl;
#endif
	default:
	  if ( can_launch_[orders->weapon_] )
	    orders->type_ = BTC_LAUNCH;
	  else
	    orders->type_ = BTC_COMBO;
	  break;
	}
	weapons_++;
	commando_.insert_after_tail( orders );
      }
      if ( weapon->token() == BT_NICE_DAY )
	next_launch_+= quantity;
    }
  }
  reloaded_ = 1;
}

void BTComputer::clearCommando() {
  BTCOrders *orders;
  while ( commando_.remove_head(orders) )
    switch(orders->type_) {
    case BTC_LAUNCH:
    case BTC_COMBO:
      delete orders;
      break;
    default:
      break;
    }
}

BTCOrders *BTComputer::activateCommando() {
  
  BTCOrders *orders,*o2;
  int ok, prev_line = -1;
  
  if (arsenal_ != weapon_manager_->arsenal_)
    reload();
  RWListIter<BTCOrders *> iter(commando_);
  iter.jump_before_head();
  do {
    ok = iter.peek_next(orders);
    if ( ok ) {
      iter.inc();
      if ( !orders ) {
	cerr << "BTComputer: nil orders" << endl;
	break;
      }  
      // We might be launching the weapons after we intended to.  For
      // instance, we would like to launch a BT_HAPPY at line 50 and
      // then BT_REAGAN at line 51.  So if we launch BT_HAPPY at line
      // 52 (\'cause we couldn\'t at 50), we want to launch BT_REAGAN at
      // line 53.
      if ( (prev_line > -1) && (orders->line_no_ > -1) &&
	   (orders->line_no_ > prev_line) ) {
	orders->line_no_ = orders->line_no_ - prev_line +
	  score_manager_->rep_.op_lines_;
      }
      // Check to see if the orders should be executed
      if ( ((orders->line_no_ > -1) && (orders->line_no_ <= score_manager_->
					rep_.op_lines_) ) ||
	   ((orders->bazaar_no_ > -1 ) && (orders->bazaar_no_
					   <= bazaar_no_)) ||
	   ((orders->my_line_no_ > -1) && (orders->my_line_no_ >= cboard_.top_))) {
	switch ( (orders->type_ & ORDER_MASK) ) {
	case BTC_LAUNCH:
	  if ( !weapon_manager_->BTActive[BT_MIRROR] ) {
	    if ( (prev_line == -1) && (orders->line_no_ > -1) )
	      prev_line = orders->line_no_;
	    // If we swap arsenals, we\'re gonna need to give up the
	    // order list (RW iter)
	    iter.remove_prev( o2 );
	    if ( orders->weapon_ == BT_SUSAN )
	      return orders;
	    launchWeapon( orders->weapon_ );
	    delete o2;
	  }
	  break;
	case BTC_LAWYERS_OFF:
	  lawyers_ = 0;
	  iter.remove_prev( o2 );
	  delete o2;
	  break;
	case BTC_CAN_PURCHASE:
	  can_purchase_[orders->type_>>4] = 1;
#ifdef WEAP_DEBUG
	  cout << "Can purchase " << (orders->type_>>4) << endl;
#endif
	  iter.remove_prev( o2 );
	  delete o2;
	  break;
	default:
	  break;
	}
      }
    }
  } while ( ok == 1 );
  return NULL;
}

void BTComputer::scanPiece() {
  piece_top_ = BT_PIECE_HEIGHT-1;
  piece_bottom_ = 0;
  int piece_height = 0;

  // these four values are optional right now -- we don't use them
  int top[BT_PIECE_WIDTH];
  int bottom[BT_PIECE_WIDTH];
  int left = BT_PIECE_WIDTH-1;
  int right = 0;
  
  for (int j = 0 ; j < BT_PIECE_WIDTH ; j++) {
    top[j] = bottom[j] = -1;
    for ( int k = 0 ; k < BT_PIECE_HEIGHT ; k++ )
      if ( piece_->isMapped(j,k) ) {
	if ( j < left )
	  left = j;
	if ( j > right )
	  right = j;
	if ( top[j] == -1 )
	  top[j] = k;
	bottom[j] = k;
	if ( k < piece_top_ )
	  piece_top_ = k;
	if ( k > piece_bottom_ )
	  piece_bottom_ = k;
      }
    if ( (top[j]!=-1) && (bottom[j]-top[j] > piece_height) )
      piece_height = bottom[j]-top[j];
  }
  piece_height++;
}

float BTComputer::computeValue( int x, int y, int o )
{
  float value;
  
  int scan_top,
    scan_bottom = scan_bottom_,
    holes = 0,
    no_tetri = 0;
  
  if ( scan_bottom_ ) {
    scan_bottom = y + scan_bottom_;
    if ( scan_bottom > BT_BOARD_HGT - 1 )
      scan_bottom = BT_BOARD_HGT - 1;
    no_tetri = 1;
  }
  else
    scan_bottom = BT_BOARD_HGT - 1;
  
  scan_top = y + BT_PIECE_HEIGHT - BT_SCAN_DEPTH;
  
  // backup board rep
  memcpy( &cboard_bak_, &cboard_, sizeof(BTCBoard) );

  if ( weapon_manager_->BTActive[BT_FEARED_WEIRD] ||
       weapon_manager_->BTActive[BT_FALL_OUT] ||
//       weapon_manager_->BTActive[BT_SO_LONG] ||  // does it sense so-long?
       weapon_manager_->BTActive[BT_NO_DICE] ||
       weapon_manager_->BTActive[BT_FOUR_BY_FOUR] ||
       weapon_manager_->BTActive[BT_NO_DICE] ||
       weapon_manager_->BTActive[BT_FORCE] ||
       weapon_manager_->BTActive[BT_BROKEN] ||
       lawyers_ ||
       weapon_manager_->BTActive[BT_BOTTLE] )
    no_tetri = 1;

  value = cboard_.eval( x, y, piece_, weapon_manager_, no_tetri,
			no_nice_day_,
			penalties_, 
			&cboard_bak_,
			0,
//		        scan_bottom ? scan_bottom : 
			BT_BOARD_HGT -1 );

  if ( no_nice_day_ && piece_->isHappy() )
    no_nice_day_--;

  if ( (value < min_) ) {

#ifdef COMP_DEBUG
    cboard_.drawMaps( board_manager_ );
    BTDebug2( "Variance: " << cboard_.variance_  << "Cov_HP: " <<
	      cboard_.cov_hole_pen_
        << " Hole_Pen: " << cboard_.hole_pen_ << " Height_Pen: " <<
	      cboard_.height_pen_
        << "\nLine_Bon: " << cboard_.line_bonus_ << " = VALUE: " << value );
    BTDebug2( " O_Holes: " << cboard_.open_holes_ << "  C_holes: "
        << cboard_.closed_holes_ );
    BTDebug2( " Scan height " << scan_bottom << "  Lowline: " << lowline_
	      << "  Top: " << cboard_.top_);
#endif

    min_ = value;
    move_.x = x;
    move_.y = y;
    move_.orientation = o;
    move_.target_x = x;
//    move_.path = path_;
/*    
    BTMove *dummy = NULL;
    move_.path.TailJump();
  // Currently, we don\'t care about keeping track of the path our
  // piece takes.
    if ( (dummy = move_.path.getPrev() ) ) {
      move_.path.getNext();
      if ( dummy->dir_ != BT_MOVE_DOWN )
	move_.path.TailInsert( x,y,BT_MOVE_DOWN );
    }
    else
      move_.path.TailInsert( x,y,BT_MOVE_DOWN );
      */
  }
  
  // restore board
  memcpy( &cboard_, &cboard_bak_, sizeof(BTCBoard) );

  return value;
}

void BTComputer::checkMove( int x, int y, int o, BT_MOVE_DIR dir )
{
  BTMove *dummy = NULL;

  if ( rescan_ )
    return;
  o %= piece_->orientations_;
  if ( ( y < BT_BOARD_HGT ) && piece_->canMoveTo(x,y)
       && piece_positions_.unchecked(x,y,o) ) {
      piece_positions_.check(x,y,o);
      if ( piece_->canMoveTo(x,y+1) ) {
	if ( piece_positions_.unchecked(x,y+1,o) ) {
	  if ( dir != BT_MOVE_DOWN ) {
//	    path_.TailInsert( x,y,BT_MOVE_DOWN );
	    checkMove( x, y+1, o, BT_MOVE_DOWN );
//	    path_.TailRemove();
	  }
	  else checkMove( x, y+1, o, BT_MOVE_DOWN );
	}
      }
      else float value = computeValue( x, y, o );
      if ( piece_positions_.unchecked(x-1,y,o) ) {
	if ( dir != BT_MOVE_LEFT ) {
//	  path_.TailInsert( x,y,BT_MOVE_LEFT );
	  checkMove( x-1, y, o, BT_MOVE_LEFT );
//	  path_.TailRemove();
	}
	else checkMove( x-1, y, o, BT_MOVE_LEFT );
      }
      if ( piece_positions_.unchecked(x+1,y,o) ) {
	if ( dir != BT_MOVE_RIGHT ) {
//	  path_.TailInsert( x,y,BT_MOVE_RIGHT );
	  checkMove( x+1, y, o, BT_MOVE_RIGHT );
//	  path_.TailRemove();
	}
	else checkMove( x+1, y, o, BT_MOVE_RIGHT );
      }
      
      if ( piece_positions_.unchecked(x,y, (o+1) % piece_->orientations_) )
	if ( piece_->rotate(0,0) )
	  {
//	    path_.TailInsert( x,y,BT_MOVE_ROTATE );
	    checkMove( x, y, (o+1) % piece_->orientations_, BT_MOVE_ROTATE );
//	    path_.TailRemove();
	    if ( ! piece_->rotate(0,1) )
	      // bad news -- no rev rotate
	      rescan_ = 1;
	  }
    }

  if ( moves_checked_++ % BTC_XMOVE_DELAY == 0 )
    // have Ernie check XEvents -- we don't want to delay game play
    handleEvents();

}

void BTComputer::decide()
{
  
  min_ = FLT_MAX;
//  path_.clear();
  
  int top;
  
  // get piece dimensions
  scanPiece();

  // rescan the board
  cboard_.rescan (board_manager_);

  // Make a preliminary board evaluation
  float val = cboard_.eval( 0, 0, NULL, weapon_manager_, 0,
			    no_nice_day_,
			    penalties_,
			    NULL,
			    0,
			    BT_BOARD_HGT-1 );
  cboard_.reset();

  // Reset rescan flag
  rescan_ = 0;
  
#ifdef COMP_DEBUG
  cout << "Rescaned:" << endl;
#endif
  
    int j, crevices = 0;

  // Now we check the board to get an idea of how bad things look.
  //
    lowline_ = BT_BOARD_HGT;
    int prev_height, next_height;
    for ( j = 0 ; j < BT_BOARD_WTH ; j ++ ) {
      if ( j )
	prev_height = cboard_.tops_[j-1];
      else
	prev_height = cboard_.tops_[1];
      if ( j < BT_BOARD_WTH-1 )
	next_height = cboard_.tops_[j+1];
      else
	next_height = prev_height;
      if ( (cboard_.tops_[j] >= prev_height+BT_CREVICE_DEPTH) ||
	   (cboard_.tops_[j] >= next_height+BT_CREVICE_DEPTH) ) {
	     crevices++;
	     if ( cboard_.tops_[j] < lowline_ )
	       lowline_ = prev_height;
	   }
    }

  // See if things are bad and, if so, set a bottom below which
  // the computer won\'t care about holes it creates \(otherwise,
  // it would never plug a huge canyon
    if ( (crevices > 2) && (cboard_.top_ < BT_LOWLINE) ) {
      scan_bottom_ = BT_SCAN_DEPTH; //  - crevices;
      lowline_ -=  BT_SMALL_CREVICE;
    }
    else
      // We\'re cool
      scan_bottom_ = 0;

  if ( cboard_.top_ >= lowline_ )
    scan_bottom_ = 0;
  
//  path_.TailInsert( def_x_, def_y_, BT_MOVE_DOWN );

  // Begin recursive check
  moves_checked_ = 0;
  checkMove( def_x_, 0, 0, BT_MOVE_NONE );
}


int BTComputer::run()
{
  // check to see if we\'re already making a decision
  if ( deciding_ )
    return 0;
  
  // Determine how many moves to make this run
  if ( super_ )
    make_move_ = 1;
  else
    make_move_++;
  
  int x, y,
    iteration = 0,
    game_over = 0,
    freeze = 0,
    cur_dir = BT_MOVE_NONE,
    j,move_ok;
  
  BTMove *move;
  removeTimer();
  
  while  ( make_move_ && ! bazaar_ && ! paused_ && game_ ) {
    deciding_ = 1;
    
    delta_y_ = 1;
    left_x_ = -1;
    right_x_ = 1;
    
    def_x_ = BT_DEFAULT_X;
    def_y_ = BT_DEFAULT_Y;

    // create piece
    if (piece_)
      piece_->reset();
    piece_ = piece_manager_->create (def_x_, def_y_);
    
    do {
      def_x_ = BT_DEFAULT_X - piece_->rot_ / 2;
    
      // Initialize the array that remembers what positions we\'ve checked
      memcpy( &piece_positions_, &piece_positions_bak_, sizeof(BTPosition) );
      
      // Try to place piece in it\'s initial spot.  If we fail, we have lost.
      if ( ! piece_->canMoveTo(def_x_,def_y_) )
//	board_manager_->clear();
	game_over = 1;
      
      if ( ! game_over ) {
	move = NULL;

	// Check to see if we\'ve gotten our arsenal stolen
	if (arsenal_ != weapon_manager_->arsenal_)
	  reload();

	// Check to see if we should launch some weapons.
	BTCOrders *orders;
	do {
	  orders = activateCommando();
	  // Currently, only an order to launch lazy susan can be returned.
	  // We will launch it.
	  if ( orders ) {
	    launchWeapon( orders->weapon_ );
	    delete orders;
	  }
	  else
	    reloaded_ = 0;
	} while ( orders || reloaded_ );
	
	
	// Decide where to put the piece.
	rescan_ = 1;
	while ( rescan_ ) {
	  rescan_ = 0;
	  decide( );
	}
	
	// Now, we have to find a place to rotate the piece before
	// we move it to it\'s place on the board.  We move it back
	// and forth on the top of the board until it can rotate.

	int bail = 0;
	cur_dir = BT_MOVE_LEFT;
	x = def_x_; y = def_y_;
	do {
	  // Move the piece to initial poistion
	  if (piece_->moveTo( x, y ))
	    move_ok = 1;
	  else
	    break;
	
	  // Rotate piece for final position
	  for ( j = 0 ; j < move_.orientation ; j++ )
	    if ( ! piece_->rotate() )
	      break;
	  // If we can\'t rotate the piece, then let\'s start over
	  if ( j < move_.orientation ) {
	    move_ok = 0;
            for ( ; j > 0 ; j-- )
              if ( ! piece_->rotate(1,1) )
		bail = 1;
	  }
	  // Let\'s try harder to get this piece in the board.
	  if (move_ok == 0 && !bail) {
	    if (cur_dir == BT_MOVE_LEFT) {
	      x--;
	      if (x == 0-BT_PIECE_WIDTH) {
		cur_dir = BT_MOVE_RIGHT;
		x = def_x_ + 1;
	      }
	    } else {
	      x++;
	      if (x == BT_BOARD_WTH)
		bail = 1;
	    }
	  }
	} while (move_ok == 0 || bail);

      	if ( move_ok && ! piece_->moveTo(move_.x,move_.y) )
	  move_ok = 0;
	if ( ! move_ok ) {  // Board syncronization problem
	  piece_->reset();
	  piece_ = piece_manager_->create (def_x_, def_y_);
	}
      }
    } while ( !move_ok && ! game_over );
  
    piece_no_++;
    
  if ( !game_over ) {
    score_manager_->rep_.score_ += BT_BOARD_HGT / 2;
    piece_manager_->dispose(piece_);
    piece_ = 0;
    board_manager_->checkLines();	
    score_manager_->update();
    if ( condor_ && (piece_no_ % update_freq_ == 0)) {
      BTBoard temp (board_manager_,upsidedown_);
      temp.motivation(BT_CONDOR);
      comm_manager_->sendBoard (&temp);
    }
  }
    if (piece_) {
      piece_->reset();
      piece_ = 0;
    }

  deciding_ = 0;
  
  if ( comm_manager_ ) {
    comm_manager_->flushStash();
    comm_manager_->flushWeapons();
  }
  
  if ( game_over )
    {
      game_ = 0;
      sendPlusMe(BT_GAME_OVER);
      return game_over;
    }
  
  if ( delay_ ) {
    id_ = DISPLAY->addTimeout(delay_, _cmptimeout_, this);
    make_move_--;
  }
  else if ( game_ )
    handleEvents();
}
return game_over;
}

void BTComputer::handleEvents() {
  XtInputMask mask;
  
  // Loop one time
  int x = 1;
  
  if ( btgame_ )
    // Check the X events once or continuously if the
    // opponent\'s piece is dropping
    while ( game_ && (btgame_->isDropping() || x) ) {
      while ( (mask = XtAppPending ( ((BTXDisplay *)DISPLAY)->app_ )) )
    	XtAppProcessEvent( ((BTXDisplay *)DISPLAY)->app_, mask );
      if ( x )
        x--;
    }
  else
    while ( (mask = XtAppPending ( ((BTXDisplay *)DISPLAY)->app_ )) ) {
      XtAppProcessEvent( ((BTXDisplay *)DISPLAY)->app_, mask );
    }
}
