/****************************************************************/
/*    NAME: Bryan Cantrill                                      */
/*    ACCT: bmc                                                 */
/*    FILE: BTPieceManager.C                                    */
/*    ASSN:                                                     */
/*    DATE: Tue Feb  8 21:36:36 1994                            */
/****************************************************************/

#include "BTConfig.H"
#include "BTBox.H"
#include "BTBoardManager.H"
#include "BTPieceManager.H"
#include "BTPiece.H"
#include "BTWeapon.H"

#define BT_DEFAULT_KEEP_PROB  .21
#define BT_EXOTIC_KEEP_PROB    .02
#define BT_DIE_KEEP_PROB      1
#define BT_BROKEN_PROB        10

BTPieceManager::BTPieceManager (BTWidget *da, BTBoardManager *board,
				BTPixmap *gimp) 
: BTRingNode(), board_ (board) {
  register int i;

  box_manager_ = new BTBoxManager (da, gimp);
  board->box_manager_ = box_manager_;

  // Initialize all of the keep probabilities
  for (i = BT_EL_PIECE; i <= BT_BOX_PIECE; i++) 
    keep_prob_[i] = BT_DEFAULT_KEEP_PROB;
  for (i = BT_WEIRD_OFFS + 1; i <= BT_MAX_PIECES; i++)
    keep_prob_[i] = 0;
  keep_prob_[BT_DIE_PIECE] = 1;
  keep_prob_[BT_HAP_PIECE] = BT_EXOTIC_KEEP_PROB;
  keep_prob_[BT_LONG_DONG_PIECE] = BT_EXOTIC_KEEP_PROB;
  hap_on_ = broken_ = old_piece_ = 0;

  // Now initialize all of the pieces...
  for (i = 0; i <= BT_MAX_PIECES; i++) 
    piece_[i] = 0;

  piece_[BT_EL_PIECE] = new BTElPiece (board_);
  piece_[BT_REL_PIECE] = new BTRevElPiece (board_);
  piece_[BT_SL_LF_PIECE] = new BTSldLftPiece (board_);
  piece_[BT_SL_RT_PIECE] = new BTSldRtPiece (board_);
  piece_[BT_LONG_PIECE] = new BTLongPiece (board_);
  piece_[BT_PLUG_PIECE] = new BTPlugPiece (board_);
  piece_[BT_BOX_PIECE] = new BTBoxPiece  (board_);
  piece_[BT_DOG_PIECE] = new BTDogPiece (board_);
  piece_[BT_RDOG_PIECE] = new BTRevDogPiece (board_);
  piece_[BT_CAP_PIECE] = new BTCapPiece (board_);
  piece_[BT_WALL_PIECE] = new BTWallPiece (board_);
  piece_[BT_TOWER_PIECE] = new BTTowerPiece (board_);
  piece_[BT_STAR_PIECE] = new BTStarPiece (board_);
  piece_[BT_WLONG_PIECE] = new BTWeirdLongPiece (board_);
  piece_[BT_DIE_PIECE] = new BTDiePiece  (board_);
  piece_[BT_LONG_DONG_PIECE] = new BTLongDongPiece (board_);
  piece_[BT_4x4_PIECE] = new BTFourByFourPiece  (board_);
  piece_[BT_HAP_PIECE] = new BTHappyPiece (board_);
}

BTPieceManager::~BTPieceManager() {
  for (int i = 0 ; i <= BT_MAX_PIECES; i++ )
    if (piece_[i])
      delete piece_[i];
  delete box_manager_;
}

void BTPieceManager::receive (BTRingPacket *packet) {
  register int i;

  switch (packet->token) {
  case BT_START: {
    for (i = BT_EL_PIECE; i <= BT_BOX_PIECE; i++) 
      keep_prob_[i] = BT_DEFAULT_KEEP_PROB;
    for (i = BT_WEIRD_OFFS + 1; i <= BT_MAX_PIECES; i++)
      keep_prob_[i] = 0;
    keep_prob_[BT_DIE_PIECE] = 1;
    keep_prob_[BT_HAP_PIECE] = .02;
    keep_prob_[BT_LONG_DONG_PIECE] = .02;
    broken_ = 0;
    hap_on_ = 0;
    break;
  }
  case BT_WPN_ON: {
    BTWeapon *wpn = (BTWeapon *) packet->data; 
    switch (wpn->token()) {
    case BT_FEARED_WEIRD: {
      for (i = BT_EL_PIECE; i <= BT_BOX_PIECE; i++) 
        keep_prob_[i] = 0;
      for (i = BT_WEIRD_OFFS + 1; i <= BT_WLONG_PIECE; i++)
        keep_prob_[i] = BT_DEFAULT_KEEP_PROB;
      break; 
    }  
    case BT_FOUR_BY_FOUR: {
      keep_prob_[BT_BOX_PIECE] = 0;
      keep_prob_[BT_4x4_PIECE] = BT_DEFAULT_KEEP_PROB;
      break;
    }
    case BT_BROKEN: {
      broken_ = 1;
      break;
    }
    case BT_NO_DICE: {
      keep_prob_[BT_DIE_PIECE] = 0;
      break;
    }
    case BT_SO_LONG: {
      keep_prob_[BT_LONG_PIECE] = 0;
      break;
    }
    case BT_NICE_DAY: {
      hap_on_++;
      break;
    }
    }
    break;
  }
  case BT_WPN_OFF: {
    BTWeapon *wpn = (BTWeapon *) packet->data; 
    switch (wpn->token()) {
    
    case BT_FEARED_WEIRD: {
      for (i = BT_EL_PIECE; i <= BT_BOX_PIECE; i++) 
        keep_prob_[i] = BT_DEFAULT_KEEP_PROB;
      for (i = BT_WEIRD_OFFS + 1; i <= BT_WLONG_PIECE; i++)
        keep_prob_[i] = 0;
      break; 
    }  
    case BT_FOUR_BY_FOUR: {
      keep_prob_[BT_BOX_PIECE] = BT_DEFAULT_KEEP_PROB;
      keep_prob_[BT_4x4_PIECE] = 0;
      break;
    }
    case BT_NO_DICE: {
      keep_prob_[BT_DIE_PIECE] = BT_DIE_KEEP_PROB;
      break;
    }
    case BT_SO_LONG: {
      keep_prob_[BT_LONG_PIECE] = BT_DEFAULT_KEEP_PROB;
      break;
    }
    case BT_BROKEN: {
      broken_ = 0;
      break;
    }
    }
    break;
  }
  }
  pass (packet);
}    

#ifdef __sun
#ifndef lrand48
static long
lrand48()
{
	return (rand());
}
#endif

#ifndef drand48
static double
drand48()
{
	return ((double)rand() / (double)RAND_MAX);
}
#endif
#endif /* __sun */

BTPiece *BTPieceManager::create (int x, int y) {
  BTPiece *new_piece;
  int i = BT_HAP_PIECE;
  double j;
  
  if (!hap_on_ && (!broken_ || (broken_ && lrand48() % BT_BROKEN_PROB == 0))) {
#ifdef BT_DEBUG_SHOW_STATS
    long k = 0;
    Block<long> stat;
    stat.Resize (BT_MAX_PIECES);
    for (i = 0; i < BT_MAX_PIECES; i++)
      stat[i] = 0;
    do {
#endif
      do {
        i = (rand() % BT_MAX_PIECES) + 1;
        j = drand48();
        if ( j < keep_prob_[i] ) break;  
      } while (1);
#ifdef BT_DEBUG_SHOW_STATS
      stat[i]++;
      if (++k %= 1000) continue;
      cerr << "\n\nNew iter.  Piece is " << i << ", stats: " << endl;
      for (i = 0; i < BT_MAX_PIECES; i++) 
        cerr << i << ": " << stat[i] << endl; 
    } while (1);
#endif
  } else if (!hap_on_ && broken_) {
    i = old_piece_;
  } else
    hap_on_--;

  assert (piece_[i]);
  old_piece_ = i;

  piece_[i]->reset();
  piece_[i]->construct (x, y);
  return piece_[i];
}

void BTPieceManager::dispose (BTPiece *old) {
  old->landed();
  old->reset();
}
