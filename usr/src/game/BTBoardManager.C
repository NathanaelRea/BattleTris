/****************************************************************/
/*    NAME: Bryan Cantrill                                      */
/*    ACCT: bmc                                                 */
/*    FILE: BTBoardManager.C                                    */
/*    ASSN:                                                     */
/*    DATE: Tue Feb  8 21:58:15 1994                            */
/****************************************************************/

#include "BTConfig.H"


#include <iostream>
using namespace std;
#include <assert.h>

#include "BattleTris.H"

#include "BTBoardManager.H"
#include "BTBox.H"
#include "BTLine.H"
#include "BTScore.H"
#include "BTBoard.H"
#include "BTDisplay.H"

BTBoardManager::BTBoardManager (BTWeaponManager *weapon, int width, int height,
                                int computer) 
: BTRingNode(), width_ (width), 
  height_ (height), idiot_ (0), upside_ (0), weapon_manager_(weapon),
  computer_(computer) {

  int i, j;

  map_ = new BTBox **[width];
  new_fill_[0] = 0;
  fill_count_ = 0;
  old_lines_ = 0;
  actives_ = 0;
  // Initialize the actual board
  for (i = 0; i < width; i++)  {
    map_[i] = new BTBox *[height];
    for (j = 0; j < height; j++) 
      map_[i][j] = 0;
  }
}

BTBoardManager::~BTBoardManager() {
  for (int i = 0; i < width_; i++)  {
    for (int j = 0; j < height_; j++) 
      if ( map_[i][j] )
	delete map_[i][j];
    delete map_[i];
  }
  delete map_;
}

void BTBoardManager::swap (int x1, int y1, int x2, int y2) {
  BTBox *temp = map_[x1][y1];
  map_[x1][y1] = map_[x2][y2];
  map_[x2][y2] = temp;

  if (map_[x2][y2]) {
    map_[x2][y2]->moveTo (x2,y2);
  }
  if (map_[x1][y1]) {
    if (!map_[x2][y2])
      map_[x1][y1]->erase(); 
    map_[x1][y1]->moveTo (x1,y1,1);
  }
}

// This is likely the swilliest swill in Swillsville

void BTBoardManager::removeLine (int line, int x1, int x2) {

  if (x2 < 0)
    x2 = width_;

  int i, j;

  // The mission here is to remove _line_.  This is done by removing
  // the line, and then dropping the rest of the board down, which
  // gets a little ugly between such weapons as Upbyside, the Force
  // and Bottleneck....

  if (!weapon_manager_->BTActive[BT_UPBYSIDE] || computer_) { 
    for (i = line; i > 0; i--) {
      if (weapon_manager_->BTActive[BT_BOTTLE]) 
      	if ((i <= BT_BOARD_HGT / 2 + BT_BOTTLE_Y) && 
	         (i >= BT_BOARD_HGT / 2 - BT_BOTTLE_Y)) {  
	       x1 = BT_BOTTLE_X;
	       x2 = width_ - BT_BOTTLE_X;
	    }
      for (j = x1; j < x2; j++) {
        if (weapon_manager_->BTActive[BT_FORCE]) {
          if (i == line && map_[j][i]) {
            map_[j][i]->erase();
	    box_manager_->dispose(map_[j][i]);
            map_[j][i] = 0;
          }
          continue;
        }
        if (map_[j][i]) {
          map_[j][i]->erase();
	  box_manager_->dispose(map_[j][i]);
	}
        map_[j][i] = map_[j][i-1];
        if (map_[j][i-1]) {
          map_[j][i-1]->erase();
          map_[j][i-1] = 0;
          map_[j][i]->moveTo (j, i);
        }
      }
    }
    for (i = x1; i < x2; i++) 
      if (!weapon_manager_->BTActive[BT_FORCE])
        map_[i][0] = 0;
  } else {
    for (i = line; i < height_ - 1; i++) {
      if (weapon_manager_->BTActive[BT_BOTTLE])
	       if ((i <= BT_BOARD_HGT / 2 + BT_BOTTLE_Y) &&
	         (i >= BT_BOARD_HGT / 2 - BT_BOTTLE_Y - 1)) {
	         x1 = BT_BOTTLE_X;
	         x2 = width_ - BT_BOTTLE_X;
	       }
      for (j = x1; j < x2; j++) {
        if (weapon_manager_->BTActive[BT_FORCE]) {
          if (i == line && map_[j][i]) {
            map_[j][i]->erase();
	    box_manager_->dispose(map_[j][i]);
            map_[j][i] = 0;
          }
          continue;
        }
        if (map_[j][i]) {
          map_[j][i]->erase();
	  box_manager_->dispose(map_[j][i]);
	}
        map_[j][i] = map_[j][i+1];
        if (map_[j][i+1]) {
          map_[j][i+1]->erase();
          map_[j][i+1] = 0;
          map_[j][i]->moveTo (j, i);
        }
      }
    }
    for (i = x1; i < x2; i++) 
      if (!weapon_manager_->BTActive[BT_FORCE])
        map_[i][height_ - 1] = 0;
  }    
}


//
// insertLine will onle be called as the result of a rise up or
// a Lawyers\' D.
//

void BTBoardManager::insertLine () {

  int x1, x2;

  int i, j;

  // Find a random hole
  int hole = rand() % width_;

  // First take care of the case where we aren\'t upside down
  if (!weapon_manager_->BTActive[BT_UPBYSIDE] || computer_) { 

    // Before we can insert the line, we need to run through the board
    // pushing everything up a line
    for (i = 0; i < height_ - 1; i++) {
      x1 = 0;  x2 = width_;

      // If BottleNeck is active and we are in the neck, then we 
      // only want to push up the width of the neck
      if (weapon_manager_->BTActive[BT_BOTTLE]) 
        if (i < BT_BOARD_HGT / 2 + BT_BOTTLE_Y) {
          x1 = BT_BOTTLE_X;
          x2 = width_ - BT_BOTTLE_X;
        }

      // Now we need to actually push up the line
      for (j = x1; j < x2; j++) {
        if (map_[j][i]) {
          map_[j][i]->erase();
	  if ( i == 0 )
	    box_manager_->dispose(map_[j][i]);
	}
        map_[j][i] = map_[j][i+1];
        if (map_[j][i+1]) {
	  // not necessary since moveTo does it
//          map_[j][i+1]->erase();
          map_[j][i+1] = 0;
          map_[j][i]->moveTo (j, i);
        }
      }
    }
    for (i = x1; i < x2; i++) {
      if (i != hole) {
        map_[i][height_ - 1] = box_manager_->create (i, height_ - 1, 6);
        map_[i][height_ - 1]->moveTo (i, height_ - 1);
      }
    }
  } else {
    // This case is symmetric.  Now we are in effect pushing the board
    // down instead of up.
    for (i = height_ - 1; i > 0; i--) {
      x1 = 0;  x2 = width_;

      // Avoid the neck
      if (weapon_manager_->BTActive[BT_BOTTLE]) 
        if (i >= BT_BOARD_HGT / 2 - BT_BOTTLE_Y) {
          x1 = BT_BOTTLE_X;
          x2 = width_ - BT_BOTTLE_X;
        }

      // Again, the actually pushing of the board
      for (j = x1; j < x2; j++) {
        if (map_[j][i])
          map_[j][i]->erase();
        map_[j][i] = map_[j][i-1];
        if (map_[j][i-1]) {
          map_[j][i-1]->erase();
          map_[j][i-1] = 0;
          map_[j][i]->moveTo (j, i);
        }
      }
    }
    for (i = x1; i < x2; i++) {
      if (i != hole) {  // Don\'t insert something in the hole 
        map_[i][0] = box_manager_->create (i, 0, BT_GREEN);
        map_[i][0]->moveTo (i, 0);
      }
    }
  }
}

void BTBoardManager::flipOnHoriz() {
  int j;
  for (int i = 0; i < height_ / 2; i++) 
    for (j = 0; j < width_; j++) 
      swap (j,i,j,height_-1-i);
}

void BTBoardManager::flipOnVert() {
  int j;
  for (int i = 0; i < width_ / 2; i++) 
    for (j = 0; j < height_; j++) 
      swap (width_-1-i,j,i,j);
}
  
void BTBoardManager::fill (int x, int y, BTBox *new_box) { 
  // Fill in the given location with the given box
  if ((x >= 0) && (x < width_) && (y >= 0) && (y < height_)) {
    map_[x][y] = new_box; 
    new_fill_[fill_count_++] = new_box;
  }
  else
    assert ( 1 == 0);
}

void BTBoardManager::receive (BTRingPacket *packet) {
  switch (packet->token) {
  case BT_START:
    new_fill_[0] = 0;
    fill_count_ = 0;
    actives_ = 0;
    old_lines_ = 0;
    upside_ = 0;
    break;
  case BT_OP_SCORE: {
    BTScore *op_score = (BTScore *) packet->data;

    // If Lawyers\' Delite is active and the number of lines has increased,
    // push the board up.
    if (op_score->lines_ > old_lines_ && weapon_manager_->BTActive[BT_LAWYERS]) {
      for (int i = 0; i < op_score->lines_ - old_lines_; i++)
        insertLine();
      redraw();
    }
    old_lines_ = op_score->lines_;
    break;
  } 

  case BT_WPN_ON: {
    actives_++;  // Why do we need this?
    BTWeapon *wpn = (BTWeapon *) packet->data; 
    switch (wpn->token()) {
    case BT_UPBYSIDE: {
      if (!upside_ && !computer_) {

        // Why do we need to do this?
        BTBoard board (this);
          
        // Flip on the horizontal axis and redraw           
        flipOnHoriz();
        redraw();
      }
      upside_ = 1;  // Can we avoid this flag?
      break;
    }

    // Bug report and Piece it together
    case BT_PIECE_IT:
    case BT_BUG: {

      // We need to create a new box at a random, empty location,
      // but, for fairness sake, we don\'t want to create it _too_ close
      // to either the top or the bottom...the missing piece will only
      // appear in the middle two quarters of the board.
      int i, j;
      do {
        i = rand() % BT_BOARD_WTH;
        j = (rand() % (BT_BOARD_HGT / 2)) + BT_BOARD_HGT / 4;  
      } while (occupied (i, j));

      if (wpn->token() == BT_BUG)
        // Create the invisible piece and draw it
        map_[i][j] = box_manager_->create (i, j, BT_INVISIBLE); 	       
      else
        // Create a new box with a random color
        map_[i][j] = box_manager_->create (i, j, rand() % (BT_NEUTRAL - 1) + 1); 	       

      map_[i][j]->moveTo (i, j);
      redraw();
    if (!computer_) DISPLAY->flush();
      break;
    }     

    // Missing Pieces
    case BT_MISSING: {

      // Need to simply locate a piece on the board and remove it

      /// Actually you need to do more than that.  You need to get a
      /// fucking clue first.
      int x,y,i, j;
      i = rand() % width_;
      j = rand() % height_;
      x = i; y = j;
      int flag = 1;
      BTBox **box = NULL;
      for ( j = y; !box && ((j!=y) || flag) ; j = (j+1) % BT_BOARD_HGT ) {
	flag = 0;
	int flag2 = 1;
	for ( i = x; !box && ((i!=x) || flag2) ; i = (i+1) % BT_BOARD_WTH ) {
	  flag2 = 0;
	  if ( map_[i][j] )
	    if ( occupied (i,j) && map_[i][j]->isRemoveable() )
	      box=&map_[i][j];
	}
      }
      if ( box ) {
	(*box)->erase();
	box_manager_->dispose (*box);
	*box = 0;
	redraw();
      }
      break;
    }

    // Blind Cleric
    case BT_BLIND: {
      //  Run through the entire board, randomly nuking half of it.
      for (int i = 0; i < height_; i++)
	      for (int j = 0; j < width_; j++)
	        if (map_[j][i] && map_[j][i]->isRemoveable()) {
		  if ((rand() % 2) == 0) {
		    map_[j][i]->erase();
		    box_manager_->dispose (map_[j][i]);
		    map_[j][i] = 0;
		  }
		}
      redraw();
      break;
    }

    case BT_GIMP: {
      for (int i = 0; i < height_; i++)
	for (int j = 0; j < width_; j++) {
	  if (map_[j][i]) {
	    int value = map_[j][i]->value();
	    box_manager_->dispose(map_[j][i]);
	    map_[j][i] = box_manager_->createGimp(j,i,value);
	  }
	}
      redraw();
      break;
    }

    // Twilight Zone
    case BT_TWILIGHT: {

      // Run through the entire board, replacing each box with an invisible box
      for (int i = 0; i < height_; i++)
	      for (int j = 0; j < width_; j++)
	        if (map_[j][i]) {
		  map_[j][i]->hide();
		  map_[j][i]->redraw();
	        }
      redraw();
      break;
    }    

    case BT_FLIP_OUT: {
      // Simply flip on the vertical axis and redraw
      flipOnVert();
      redraw();
      break;
    }

    case BT_FALL_OUT: {
      // For every line, remove the bottom line and redraw.  This will have
      // the effect of the middle of the board "falling out"
      for (int i = 0; i < height_; i++) {
        if (!weapon_manager_->BTActive[BT_UPBYSIDE] || computer_) 
          removeLine (height_ - 1, BT_FALL_OUT_LEDGE, width_ - BT_FALL_OUT_LEDGE);
        else
          removeLine (0, BT_FALL_OUT_LEDGE, width_ - BT_FALL_OUT_LEDGE);
        redraw(); 
      }
      break;
    }

    case BT_BOTTLE: {
      // Run through the area of the neck, overwriting the boxes with 
      // neutral boxes
      for (int x = 0; x < BT_BOTTLE_X; x++) {
        for (int y = BT_BOARD_HGT / 2 - BT_BOTTLE_Y; 
          y < BT_BOARD_HGT / 2 + BT_BOTTLE_Y; y++) {
            if (map_[x][y]) 
              box_manager_->dispose (map_[x][y]);
	    map_[x][y] = box_manager_->structureCreate(x,y);
            map_[x][y]->redraw();
            if (map_[width_ - x - 1][y])
              box_manager_->dispose (map_[width_ - x - 1][y]);
            map_[width_ - x - 1][y] = 
              box_manager_->structureCreate (width_ - x - 1, y);
            map_[width_ - x - 1][y]->redraw();
         }
      }
      checkLines();
      break;
    }

    case BT_RISE_UP: {
      // Trivial
      insertLine();
      redraw();
      break;
    }
    }
    break;
  } // case BT_WPN_ON

  case BT_WPN_OFF: {
    actives_--;
    BTWeapon *wpn = (BTWeapon *) packet->data; 
    switch (wpn->token()) {

    case BT_UPBYSIDE: {
      // Need to flip this back...
      upside_ = 0;
      if ( !computer_ ) {
        flipOnHoriz();
        redraw();
      }
      break;
    }

    case BT_BOTTLE: {
      // Need to undo the bottle neck and replace it with dead space
      for (int x = 0; x < BT_BOTTLE_X; x++) {
        for (int y = BT_BOARD_HGT / 2 - BT_BOTTLE_Y; 
          y < BT_BOARD_HGT / 2 + BT_BOTTLE_Y; y++) {
	  assert(map_[x][y]);  // the bottle neck better be there
            map_[x][y]->erase();
            box_manager_->dispose (map_[x][y]);
            map_[x][y] = 0;
	  assert(map_[width_-x-1][y]);
            map_[width_ - x - 1][y]->erase();
            box_manager_->dispose (map_[width_ - x - 1][y]);
            map_[width_ - x -1][y] = 0;
        }
      }
      redraw();
      break;
    }
    default:
      break;
    }
  }
  }
  pass (packet);
}

void BTBoardManager::landed(int x, int y) {

  // Need to determine if the player is an idiot...this is done by 
  // finding an empty slot which is completely surrounded by pieces.
  for (int i = 0; i < BT_PIECE_WIDTH; i++) {
    for (int j = 0; j < BT_PIECE_HEIGHT; j++) { 
      // Found one that\'s empty...
      if (!occupied(x+i,y+j)) {
        BTBox *left = 0, *right = 0, *top = 0;
          
        // First check to see if it is surrounded on three sides
        if (!occupied(x+i-1, y+j)) continue;

        if ((x+i-1 >= 0) && (x+i-1 < width_) && (y+j >= 0) && (y+j < height_))
          left = map_[x+i-1][y+j];

        if (!occupied(x+i+1, y+j)) continue;

        if ((x+i+1 >= 0) && (x+i+1 < width_) && (y+j >= 0) && (y+j < height_))
          right = map_[x+i+1][y+j];

        if (!weapon_manager_->BTActive[BT_UPBYSIDE]) {
          if (!occupied(x+i  , y+j-1)) continue;
          if ((x+i >= 0) && (x+i < width_) && (y+j-1 >= 0) && (y+j-1 < height_))
             top = map_[x+i][y+j-1];
        } else {
          if (!occupied(x+i  , y+j+1)) continue;
          if ((x+i >= 0) && (x+i < width_) && (y+j+1 >= 0) && (y+j+1 < height_))
             top = map_[x+i][y+j+1];
        }

        // If we are here then we are surrounded on both left, right
        // and logical top by pieces...the player may be an idiot.  Now
        // we need to see if any of the pieces are the result of a new
        // placement
        for (int k = 0; k < fill_count_; k++) {
          assert (new_fill_[k]);
          if (new_fill_[k] == left || new_fill_[k] == right 
            || new_fill_[k] == top) {
              idiot_ = 1;
              reason_ = BT_BAD_MOVE;
              break;
          }
        }
      }
    }
  }

  new_fill_[0] = 0;
  fill_count_ = 0;
} 
  
int BTBoardManager::checkLines() {
  int i, j, k;
  short value = 0, nvalue = 0;
  BTLine lines;
  short min = height_ - 1;

  // Before we kill check for lines, look to see how high the board is 
  // (i.e. how much space we have at the top).  This will be used to
  // determine the probability that a near-death sound is sent around
  // the board.
  for (j = height_ - 1; j > 0; j--) {    // Note:  This needs to work for UD
    for (i = 0; i < width_; i++) {
      if (map_[i][j]) {
        if (j < min)
          min = j;
        break;
      }
    }
    if (i == width_) break;
  }

  // Now we need to actually run through the board looking for complete 
  // lines.
  for (j = height_ - 1; j >= 0; j--) {
    nvalue = 0;

    for (i = 0; i < width_ && map_[i][j]; i++) 
      nvalue += map_[i][j]->value();

    if (i == width_) {
      // We\'ve got a line...add nvalue to the value.
      ++lines;
      value += nvalue;
      removeLine (j, 0, width_);
      if (!weapon_manager_->BTActive[BT_FORCE])
        j++;
    } else {
      // We don\'t have a line.  Check to see if there is a happy face here
      // (and turn it into a saddy face if there is)
      for (i = 0; i < width_; i++) {
        if (map_[i][j] && (map_[i][j]->value() == BT_HAPPY_VAL)) {
          map_[i][j]->landed();
          idiot_ = 1;
          reason_ = BT_MISSED_SMILEY;
        }
      }
    }
  }

  // Need to come with something more creative here
  if (min < 8) {
    idiot_ = 1;
    reason_ = BT_NEAR_DEATH;
  }

  if (!lines) 
    return 0;

  // Clear the idiot light
  idiot_ = 0;
  redraw();

  short funds = value * lines.inc();
  send (BT_FUNDS, &funds);
  send (BT_LINE, &lines);
  return (value * lines.inc());
}

void BTBoardManager::redraw() {
  for (int i = 0; i < height_; i++)
    for (int j = 0; j < width_; j++)
      if (map_[j][i]) 
        map_[j][i]->redraw();
  if (!computer_) DISPLAY->flush();
}

void BTBoardManager::newBoard (BTBoard *board) {
  for (int i = 0; i < height_; i++)
    for (int j = 0; j < width_; j++) {
      if (map_[j][i]) {
        map_[j][i]->erase();
        box_manager_->dispose (map_[j][i]);
      }
      map_[j][i] = 0;
      if (board->rep_[i * width_ + j]) {
        map_[j][i] = box_manager_->createByID (j, i, board->rep_[i*width_+j]);
        map_[j][i]->redraw();
      }
    }
}

void BTBoardManager::clear() {
  for (int i = 0; i < height_; i++)
    for (int j = 0; j < width_; j++) {
      if (map_[j][i]) {
//	map_[j][i]->erase();  ?? Why is this commented out ??
        box_manager_->dispose (map_[j][i]);
      }
      map_[j][i] = 0;
    }
}
