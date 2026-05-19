/****************************************************************/
/*    NAME: Bryan Cantrill                                      */
/*    ACCT: bmc                                                 */
/*    FILE: BTPiece.C                                           */
/*    ASSN:                                                     */
/*    DATE: Sun Feb  6 23:39:00 1994                            */
/****************************************************************/

#include "BTConfig.H"

#if STDC_HEADERS
# include <stdlib.h>
#endif


#include <iostream>
using namespace std;
#include <assert.h>

#include "BTPiece.H"
#include "BTBox.H"
#include "BTBoardManager.H"

BTPiece::BTPiece (int color, BTBoardManager *board) 
: color_ (color + 1), board_ (board), rot_ (3)  {
  x_ = y_ = placed_ = 0;
  for (int i = 0; i < BT_PIECE_WIDTH; i++)
    for (int j = 0; j < BT_PIECE_HEIGHT; j++)
      map_[i][j] = 0;
  orientation_ = 0;
  orientations_ = 4;
}

// When passed an x and a y value, isMapped will just let you know
// if this is a 1 or a 0 in the piece map.
int BTPiece::isMapped (int x, int y) {
  if (map_[x][y])
    return 1;
  else return 0;
}

// Since Ernie needs to know if a piece _can_ move to a particular
// location without actually moving it, we have canMoveTo().

int BTPiece::canMoveTo (int x, int y) {
  for (int i = 0; i < BT_PIECE_WIDTH; i++) 
    for (int j = 0; j < BT_PIECE_HEIGHT; j++) {
      if (map_[i][j] && board_->occupied (x + i, y + j))
        return 0;
    }
  return 1;
}    
 
// moveTo moves the piece if it can (if it can't, it returns 0).

int BTPiece::moveTo (int x, int y) {

  int i, j;

  // First check to see if we can move the piece
  for (i = 0; i < BT_PIECE_WIDTH; i++) {
    for (j = 0; j < BT_PIECE_HEIGHT; j++) {
      if (map_[i][j] && board_->occupied (x + i, y + j))
        return 0;
    }
  }

  // If we're here, we can move the piece, so move it over, box by box
  x_ = x; y_ = y;
  for (i = 0; i < BT_PIECE_WIDTH; i++) {
    for (j = 0; j < BT_PIECE_HEIGHT; j++) {
      if (map_[i][j]) {
        map_[i][j]->moveTo (x_ + i, y_ + j, !placed_);
      }
    }
  }

  placed_ = 1;
  redraw();
  return 1;
}

// Just as we have canMoveTo and moveTo, we have canRotate and rotate...

int BTPiece::canRotate( int x, int y ) {

  // Dies, happy faces, etc. cannot rotate...
  if (!rot_) return 0;

  // Run through the piece map making requests to the board to
  // see if this piece can rotate
  for (int i = 0; i < rot_; i++) {
    for (int j = 0; j < rot_; j++) {
      if (map_[rot_-1-j][i] && board_->occupied (x + i, y + j))
        return 0;
    }
  }
  return 1;
}

int BTPiece::rotate ( int redraw, int reverse ) {
  if (!rot_) return 0;

  // rotate creates a new map of the rotated piece, and then
  // superimposes that map onto the board if they're are no conflicts.
  BTBox *rot_map[BT_PIECE_WIDTH][BT_PIECE_HEIGHT];
  int i, j;

  // First run through the piece map, rotating onto the rot_map
  for (i = 0; i < rot_; i++) {
    for (j = 0; j < rot_; j++) {

      // Check to see if we're rotating counter-clockwise or not
      if ( reverse )
        rot_map[i][j] = map_[j][rot_-1-i];
      else
        rot_map[i][j] = map_[rot_-1-j][i];

      // Check for conflicts with the board
      if (rot_map[i][j] && board_->occupied (x_ + i, y_ + j))
        return 0;
    }
  }

  // If we're here, then the rotate is legal eagle.  Superimpose
  // it onto the board.

  for (i = 0; i < rot_; i++) {
    for (j = 0; j < rot_; j++) {
      map_[i][j] = rot_map[i][j];
      if ( map_[i][j]) {
        map_[i][j]->moveTo (x_ + i, y_ + j);
      }
    }
  }

  if ( redraw )
    BTPiece::redraw();

  // Update our orientation.
  if ( reverse ) 
    orientation_ = (orientation_ - 1) % 4;
  else 
    orientation_ = (orientation_ + 1) % 4;

  return 1;
}
    
void BTPiece::reset() {
  x_ = y_ = orientation_ = placed_ = 0;
  for (int i = 0; i < BT_PIECE_WIDTH; i++) 
    for (int j = 0; j < BT_PIECE_HEIGHT; j++)  {
      if ( map_[i][j] )
	board_->box_manager_->dispose(map_[i][j]);
      map_[i][j] = 0;
    }
}

// Flips our piece right-side up.
void BTPiece::resetOrientation() {
  while ( orientation_ )
    rotate(0);
}

void BTPiece::redraw() {
  for (int i = 0; i < BT_PIECE_WIDTH; i++) 
    for (int j = 0; j < BT_PIECE_HEIGHT; j++) 
      if (map_[i][j])
        map_[i][j]->redraw();
}

void BTPiece::landed() {
  // No sweat...just fill in our position in the board.
  for (int i = 0; i < BT_PIECE_WIDTH; i++) 
    for (int j = 0; j < BT_PIECE_HEIGHT; j++) 
      if (map_[i][j]) {
        board_->fill (x_ + i, y_ + j, map_[i][j]);
	map_[i][j] = 0;
      }
  board_->landed (x_, y_);   
}

BTPiece::~BTPiece() {}
  
BTElPiece::BTElPiece (BTBoardManager *board) 
: BTPiece (BT_EL_PIECE, board) {}

void BTElPiece::construct (int x, int y) {
  x_ = x;  y_ = y;
  map_[1][0] = board_->box_manager_->create (x_+1, y_,   color_);
  map_[1][1] = board_->box_manager_->create (x_+1, y_+1, color_);
  map_[1][2] = board_->box_manager_->create (x_+1, y_+2, color_);
  map_[2][2] = board_->box_manager_->create (x_+2, y_+2, color_);
}

BTRevElPiece::BTRevElPiece (BTBoardManager *board) 
: BTPiece (BT_REL_PIECE, board) {}

void BTRevElPiece::construct (int x, int y) {
  x_ = x;  y_ = y;
  map_[2][0] = board_->box_manager_->create (x_+2, y_,   color_);
  map_[2][1] = board_->box_manager_->create (x_+2, y_+1, color_);
  map_[2][2] = board_->box_manager_->create (x_+2, y_+2, color_);
  map_[1][2] = board_->box_manager_->create (x_+1, y_+2, color_);
}

BTSldLftPiece::BTSldLftPiece (BTBoardManager *board) 
: BTPiece (BT_SL_LF_PIECE, board) {}

void BTSldLftPiece::construct (int x, int y) {
  x_ = x;  y_ = y;
  map_[0][1] = board_->box_manager_->create (x_,   y_+1, color_);
  map_[1][1] = board_->box_manager_->create (x_+1, y_+1, color_);
  map_[1][2] = board_->box_manager_->create (x_+1, y_+2, color_);
  map_[2][2] = board_->box_manager_->create (x_+2, y_+2, color_);
}

BTSldRtPiece::BTSldRtPiece (BTBoardManager *board) 
: BTPiece (BT_SL_RT_PIECE, board) {}

void BTSldRtPiece::construct (int x, int y) {
  x_ = x;  y_ = y;
  map_[0][2] = board_->box_manager_->create (x_+0, y_+2, color_);
  map_[1][2] = board_->box_manager_->create (x_+1, y_+2, color_);
  map_[1][1] = board_->box_manager_->create (x_+1, y_+1, color_);
  map_[2][1] = board_->box_manager_->create (x_+2, y_+1, color_);
}

BTLongPiece::BTLongPiece (BTBoardManager *board) 
: BTPiece (BT_LONG_PIECE, board) {
  rot_ = 4;
}

void BTLongPiece::construct (int x, int y) {
  x_ = x;  y_ = y;
  map_[0][1] = board_->box_manager_->create (x_+0, y_+1, color_);
  map_[1][1] = board_->box_manager_->create (x_+1, y_+1, color_);
  map_[2][1] = board_->box_manager_->create (x_+2, y_+1, color_);
  map_[3][1] = board_->box_manager_->create (x_+3, y_+1, color_);
}

BTPlugPiece::BTPlugPiece (BTBoardManager *board) 
: BTPiece (BT_PLUG_PIECE, board) {}

void BTPlugPiece::construct (int x, int y) {
  x_ = x;  y_ = y;
  map_[0][2] = board_->box_manager_->create (x_+0, y_+2, color_);
  map_[1][2] = board_->box_manager_->create (x_+1, y_+2, color_);
  map_[1][1] = board_->box_manager_->create (x_+1, y_+1, color_);
  map_[2][2] = board_->box_manager_->create (x_+2, y_+2, color_);
}

BTBoxPiece::BTBoxPiece (BTBoardManager *board) 
: BTPiece (BT_BOX_PIECE, board) {
  rot_ = 0;
}

void BTBoxPiece::construct (int x, int y) {
  x_ = x;  y_ = y;
  map_[1][1] = board_->box_manager_->create (x_+1, y_+1, color_);
  map_[1][2] = board_->box_manager_->create (x_+1, y_+2, color_);
  map_[2][1] = board_->box_manager_->create (x_+2, y_+1, color_);
  map_[2][2] = board_->box_manager_->create (x_+2, y_+2, color_);
}

BTDiePiece::BTDiePiece (BTBoardManager *board) 
: BTPiece (BT_DIE_PIECE, board) { 
  rot_ = 0;
  color_ = BT_IVORY;
}

void BTDiePiece::construct (int x, int y) {
  x_ = x;  y_ = y;
  map_[1][1] = board_->box_manager_->dieCreate (x_+1,y_+1, rand() % 6 + 1);
}  

BTHappyPiece::BTHappyPiece (BTBoardManager *board) 
: BTPiece (BT_HAP_PIECE, board) { 
  rot_ = 0;
  color_ = BT_IVORY;
}

void BTHappyPiece::construct (int x, int y) {
  x_ = x;  y_ = y;
  map_[1][1] = board_->box_manager_->happyCreate (x_+1,y_+1);
}  

BTDogPiece::BTDogPiece (BTBoardManager *board) 
: BTPiece (BT_DOG_PIECE - BT_WEIRD_OFFS, board) {}

void BTDogPiece::construct (int x, int y) {
  x_ = x;  y_ = y;
  map_[0][0] = board_->box_manager_->create (x_+0, y_+0, color_);
  map_[1][1] = board_->box_manager_->create (x_+1, y_+1, color_);
  map_[2][1] = board_->box_manager_->create (x_+2, y_+1, color_);
  map_[2][2] = board_->box_manager_->create (x_+2, y_+2, color_);
}

BTRevDogPiece::BTRevDogPiece (BTBoardManager *board) 
: BTPiece (BT_RDOG_PIECE - BT_WEIRD_OFFS, board) {}

void BTRevDogPiece::construct (int x, int y) {
  x_ = x;  y_ = y;
  map_[0][1] = board_->box_manager_->create (x_+0, y_+1, color_);
  map_[0][2] = board_->box_manager_->create (x_+0, y_+2, color_);
  map_[1][1] = board_->box_manager_->create (x_+1, y_+1, color_);
  map_[2][2] = board_->box_manager_->create (x_+2, y_+2, color_);
}

BTCapPiece::BTCapPiece (BTBoardManager *board) 
: BTPiece (BT_CAP_PIECE - BT_WEIRD_OFFS, board) {
  rot_ = 4;
}

void BTCapPiece::construct (int x, int y) {
  x_ = x;  y_ = y;
  map_[0][2] = board_->box_manager_->create (x_+0, y_+2, color_);
  map_[1][1] = board_->box_manager_->create (x_+1, y_+1, color_);
  map_[2][1] = board_->box_manager_->create (x_+2, y_+1, color_);
  map_[3][2] = board_->box_manager_->create (x_+3, y_+2, color_);
}

BTWallPiece::BTWallPiece (BTBoardManager *board) 
: BTPiece (BT_WALL_PIECE - BT_WEIRD_OFFS, board) {
  rot_ = 4;
  state_ = 0;
}

void BTWallPiece::construct (int x, int y) {
  state_ = 0;
  x_ = x;  y_ = y;
  map_[0][1] = board_->box_manager_->create (x_+0, y_+1, color_);
  map_[0][2] = board_->box_manager_->create (x_+0, y_+2, color_);
  map_[3][1] = board_->box_manager_->create (x_+3, y_+1, color_);
  map_[3][2] = board_->box_manager_->create (x_+3, y_+2, color_);
}

int BTWallPiece::rotate (int redraw, int reverse )
{
  int new_state;

  // Need to be able to reverse rotate 
  if ( reverse )
      new_state = (state_ - 1 + orientations_) % orientations_;
    else
      new_state = (state_ + 1) % orientations_;
    switch (new_state) {
    case 0:
      if ( !reverse ) {
	if ( (board_->occupied(x_,y_+2) || board_->occupied(x_+3,y_+1)) )
	  return 0;    
	map_[0][2] = map_[1][0];  map_[1][0] = 0;
	map_[3][1] = map_[2][3];  map_[2][3] = 0;
      }
      else {
	if ( (board_->occupied(x_,y_+1) || board_->occupied(x_+3,y_+2)) )
	  return 0;
	map_[0][1] = map_[1][3];  map_[1][3] = 0;
	map_[3][2] = map_[2][0];  map_[2][0] = 0;
      }
      break;
    case 1:
      if ( !reverse ) {
	if ( (board_->occupied(x_+1,y_+3) || board_->occupied(x_+2,y_)) )
	  return 0;    
	map_[1][3] = map_[0][1];  map_[0][1] = 0;
	map_[2][0] = map_[3][2];  map_[3][2] = 0;
      }
      else {
	if ( (board_->occupied(x_,y_+2) || board_->occupied(x_+3,y_+1)) )
	  return 0;    
	map_[0][2] = map_[2][3]; map_[2][3] = 0;
	map_[3][1] = map_[1][0]; map_[1][0] = 0;
      }
      break;
    case 2:
      if ( !reverse ) {
	if ( (board_->occupied(x_+2,y_+3) || board_->occupied(x_+1,y_)) )
	  return 0;    
	map_[2][3] = map_[0][2];  map_[0][2] = 0;
	map_[1][0] = map_[3][1];  map_[3][1] = 0;
      }
      else {
	if ( (board_->occupied(x_+2,y_) || board_->occupied(x_+1,y_+3)) )
	  return 0;    
	map_[2][0] = map_[0][1];  map_[0][1] = 0;
	map_[1][3] = map_[3][2];  map_[3][2] = 0;
      }
      break;
    case 3:
      if ( !reverse ) {
	if ( (board_->occupied(x_,y_+1) || board_->occupied(x_+3,y_+2)) )
	  return 0;    
	map_[0][1] = map_[2][0];  map_[2][0] = 0;
	map_[3][2] = map_[1][3];  map_[1][3] = 0;
      }
      else {
	if ( (board_->occupied(x_+1,y_) || board_->occupied(x_+2,y_+3)) )
	  return 0;    
	map_[1][0] = map_[0][2];  map_[0][2] = 0;
	map_[2][3] = map_[3][1];  map_[3][1] = 0;
      }
      break;
    default:
      break;
    }
    state_ = new_state;


  for (int i = 0; i < BT_PIECE_WIDTH; i++) 
    for (int j = 0; j < BT_PIECE_HEIGHT; j++) {
      if ( map_[i][j]) {
        map_[i][j]->moveTo (x_ + i, y_ + j);
      }
    }

  // Update our orientation.
  if ( reverse ) 
    orientation_ = (orientation_ - 1) % orientations_;
  else 
    orientation_ = (orientation_ + 1) % orientations_;

  if (redraw) 
    BTPiece::redraw();
  return 1;
}

BTTowerPiece::BTTowerPiece (BTBoardManager *board) 
: BTPiece (BT_TOWER_PIECE - BT_WEIRD_OFFS, board) {}

void BTTowerPiece::construct (int x, int y) {
  x_ = x;  y_ = y;
  map_[2][0] = board_->box_manager_->create (x_+2, y_+0, color_);
  map_[1][1] = board_->box_manager_->create (x_+1, y_+1, color_);
  map_[0][1] = board_->box_manager_->create (x_+0, y_+1, color_);
  map_[2][2] = board_->box_manager_->create (x_+2, y_+2, color_);
}

BTStarPiece::BTStarPiece (BTBoardManager *board) 
: BTPiece (BT_STAR_PIECE - BT_WEIRD_OFFS, board) {
  state_ = 0;
  orientations_ = 2;
}

void BTStarPiece::construct (int x, int y) {
  state_ = 0;
  x_ = x;  y_ = y;
  map_[1][0] = board_->box_manager_->create (x_+1, y_+0, color_);
  map_[0][1] = board_->box_manager_->create (x_+0, y_+1, color_);
  map_[1][2] = board_->box_manager_->create (x_+1, y_+2, color_);
  map_[2][1] = board_->box_manager_->create (x_+2, y_+1, color_);
}

int BTStarPiece::rotate(int redraw, int reverse ) {

  // Swilly-swill on the window sill is the price of cool rotation
  if (!state_) {
    if (board_->occupied(x_,y_) || board_->occupied(x_+2,y_) ||
        board_->occupied(x_,y_+2) || board_->occupied(x_+2,y_+2))
      return 0;
    map_[0][0] = map_[1][0]; map_[1][0] = 0;
    map_[2][0] = map_[2][1]; map_[2][1] = 0;
    map_[0][2] = map_[0][1]; map_[0][1] = 0;
    map_[2][2] = map_[1][2]; map_[1][2] = 0;
  } else {
    if (board_->occupied(x_+1,y_) || board_->occupied(x_,y_+1) ||
        board_->occupied(x_+1,y_+2) || board_->occupied(x_+2,y_+1))
      return 0;
    map_[1][0] = map_[0][0]; map_[0][0] = 0;
    map_[0][1] = map_[2][0]; map_[2][0] = 0;
    map_[1][2] = map_[0][2]; map_[0][2] = 0;
    map_[2][1] = map_[2][2]; map_[2][2] = 0;
  }
  state_ = (state_ + 1) % 2;

  for (int i = 0; i < BT_PIECE_WIDTH; i++) 
    for (int j = 0; j < BT_PIECE_HEIGHT; j++) {
      if ( map_[i][j]) {
        map_[i][j]->moveTo (x_ + i, y_ + j);
      }
    }
  if (redraw)
    BTPiece::redraw();
  return 1;
}

BTWeirdLongPiece::BTWeirdLongPiece (BTBoardManager *board) 
: BTPiece (BT_WLONG_PIECE - BT_WEIRD_OFFS, board) {
  state_ = 0;
  rot_ = 4;
  orientations_ = 6;
}

void BTWeirdLongPiece::construct (int x, int y) {
  x_ = x;  y_ = y;
  state_ = 0;
  map_[1][0] = board_->box_manager_->create (x_+0, y_+0, color_);
  map_[1][1] = board_->box_manager_->create (x_+1, y_+1, color_);
  map_[2][2] = board_->box_manager_->create (x_+2, y_+2, color_);
  map_[2][3] = board_->box_manager_->create (x_+3, y_+3, color_);
}

int BTWeirdLongPiece::rotate (int redraw, int reverse )
{
  int new_state, da_end = 1;
  if (reverse)
    new_state = (state_ - 1 + orientations_) % orientations_; 
  else
    new_state = (state_ + 1) % orientations_; 

  switch (new_state) {
  case 0:
    if ( !reverse ) {
      if ( board_->occupied(x_+1,y_) || board_->occupied(x_+1,y_+1) ||
	   board_->occupied(x_+2,y_+2) || board_->occupied (x_+2,y_+3))
	return 0;     
      map_[1][0] = map_[2][0];  map_[2][0] = 0;
      map_[1][1] = map_[2][1];  map_[2][1] = 0;
      map_[2][2] = map_[1][2];  map_[1][2] = 0;
      map_[2][3] = map_[1][3];  map_[1][3] = 0;
    }
    else {
      if (board_->occupied(x_+1,y_) || board_->occupied(x_+2,y_+3))
	return 0;    
      map_[1][0] = map_[0][0];  map_[0][0] = 0;
      map_[2][3] = map_[3][3];  map_[3][3] = 0;
    }
    break;
    
  case 1:
    if (!reverse) {
      if (board_->occupied(x_,y_) || board_->occupied(x_+3,y_+3))
	return 0;    
      map_[0][0] = map_[1][0];  map_[1][0] = 0;
      map_[3][3] = map_[2][3];  map_[2][3] = 0;
    }
    else {
      if ( board_->occupied(x_,y_) || board_->occupied(x_+3,y_+3) )
	return 0;    
      map_[0][0] = map_[0][1];  map_[0][1] = 0;
      map_[3][3] = map_[3][2];  map_[3][2] = 0;
    }
    break;
  case 2:
    if ( !reverse ) {
      if ( board_->occupied(x_,y_+1) || board_->occupied(x_+3,y_+2) )
	return 0;    
      map_[0][1] = map_[0][0];  map_[0][0] = 0;
      map_[3][2] = map_[3][3];  map_[3][3] = 0;
    }
    else {
      if (board_->occupied(x_,y_+1) || board_->occupied(x_+1,y_+1) ||
	  board_->occupied(x_+2,y_+2) || board_->occupied (x_+3,y_+2))
	return 0;    
      map_[0][1] = map_[0][2];  map_[0][2] = 0;
      map_[1][1] = map_[1][2];  map_[1][2] = 0;
      map_[2][2] = map_[2][1];  map_[2][1] = 0;
      map_[3][2] = map_[3][1];  map_[3][1] = 0;
    }
    break;
  case 3:
    if ( !reverse ) {
      if (board_->occupied(x_,y_+2) || board_->occupied(x_+1,y_+2) ||
	  board_->occupied(x_+2,y_+1) || board_->occupied (x_+3,y_+1))
	return 0;    
      map_[0][2] = map_[0][1];  map_[0][1] = 0;
      map_[1][2] = map_[1][1];  map_[1][1] = 0;
      map_[2][1] = map_[2][2];  map_[2][2] = 0;
      map_[3][1] = map_[3][2];  map_[3][2] = 0;
    }
    else {
      if (board_->occupied(x_+3,y_+1) || board_->occupied(x_,y_+2))
	return 0;    
      map_[3][1] = map_[3][0];  map_[3][0] = 0;
      map_[0][2] = map_[0][3];  map_[0][3] = 0;
    }
    break;
  case 4:
    if (!reverse) {
      if (board_->occupied(x_+3,y_) || board_->occupied(x_,y_+3))
	return 0;    
      map_[3][0] = map_[3][1];  map_[3][1] = 0;
      map_[0][3] = map_[0][2];  map_[0][2] = 0;
    }
    else {
      if (board_->occupied(x_+3,y_) || board_->occupied(x_,y_+3))
	return 0;    
      map_[3][0] = map_[2][0];  map_[2][0] = 0;
      map_[0][3] = map_[1][3];  map_[1][3] = 0;
    }
    break;
  case 5:
    if (!reverse) {
      if (board_->occupied(x_+2,y_) || board_->occupied(x_+1,y_+3))
	return 0;    
      map_[2][0] = map_[3][0];  map_[3][0] = 0;
      map_[1][3] = map_[0][3];  map_[0][3] = 0;
    }
    else {
      if ( board_->occupied(x_+2,y_) || board_->occupied(x_+2,y_+1) ||
	   board_->occupied(x_+1,y_+2) || board_->occupied (x_+1,y_+3))
	return 0;     
      map_[2][0] = map_[1][0];  map_[1][0] = 0;
      map_[2][1] = map_[1][1];  map_[1][1] = 0;
      map_[1][2] = map_[2][2];  map_[2][2] = 0;
      map_[1][3] = map_[2][3];  map_[2][3] = 0;
    }
    break;
  default:
    break;
  }

  state_ = new_state;
  
  
  // Now that we\'ve rotated the beast, go ahead and actually move everything
  for (int i = 0; i < BT_PIECE_WIDTH; i++) 
    for (int j = 0; j < BT_PIECE_HEIGHT; j++) {
      if ( map_[i][j]) {
        map_[i][j]->moveTo (x_ + i, y_ + j);
      }
    }

  // Update our orientation.
  if ( reverse ) 
    orientation_ = (orientation_ - 1 + orientations_) % orientations_;
  else 
    orientation_ = (orientation_ + 1) % orientations_;

  if (redraw) 
    BTPiece::redraw();
  return 1;
}

BTFourByFourPiece::BTFourByFourPiece (BTBoardManager *board) 
: BTPiece (BT_BOX_PIECE, board) {
  rot_ = 0;
}

void BTFourByFourPiece::construct (int x, int y) {
  int i;

  x_ = x;  y_ = y;

  for (i = 0; i < 4; i++) {
    map_[i][0] = board_->box_manager_->create (x_+i, y_+0, color_);
    map_[i][3] = board_->box_manager_->create (x_+i, y_+3, color_);
  }
  for (i = 1; i < 3; i++) {
    map_[0][i] = board_->box_manager_->create (x_+0, y_+i, color_);
    map_[3][i] = board_->box_manager_->create (x_+3, y_+i, color_);
  }
}

BTLongDongPiece::BTLongDongPiece (BTBoardManager *board)
: BTPiece (BT_LONG_PIECE, board) {
  rot_ = 8;
}

void BTLongDongPiece::construct (int x, int y) {
  x_ = x;  y_ = y;
  for (int i = 0; i < 8; i++) 
    map_[i][0] = board_->box_manager_->create (x_+i, y_+0, color_);
}
