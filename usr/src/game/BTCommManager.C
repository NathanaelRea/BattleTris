/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: BTCommManager.C                                     */
/*    DATE: Wed Apr 13 16:56:47 1994                            */
/****************************************************************/

#include "BTConfig.H"

#include <assert.h>

#include <iostream>
using namespace std;

#include "BTStartup.H"
#include "BTCommManager.H"
#include "BTProtocol.H"
#include "BTNetwork.H"
#include "BTMessageDlog.H"
#include "StreamSocketErr.H"
#include "BTComputer.H"
#include "XtSocketCB.H"
#include "BTXDisplay.H"

const int BTCOMM_START_TIMEOUT = 60;
const char *BTCOMM_COMPUTER_NAME = "Greased Ernie";

BTCommManager::BTCommManager(BTWidget *widget, BTPimp *pimp, 
			     BTStartup *startup)
: sock_(0), widget_(widget), board_buf_(0), pimp_(pimp), startup_(startup),
  cain_(0), able_ (0), computer_(0)
{
  gamecb_ = new XtSocketCB(((BTXDisplay *)DISPLAY)->app_, gameCallback, this);
}


void BTCommManager::receive(BTRingPacket *packet)
{
  short err;

  if(!(cain_ || sock_)) {
    pass(packet);
    return;
  }

  if(cain_) {
    receiveFromLocal(packet);
    return;
  }

#ifndef NDEBUG
  if(sock_ == 0) {
    Pass(packet);
    return;
  }
#endif

  switch(packet->token) {

  case BT_WPN_LAUNCH: {
    BTWeapon *wpn = (BTWeapon *) packet->data;
    BTDebug("received weapon " << wpn->token());
    sendWeapon(wpn);
    break;
  }

  case BT_SCORE: {
    BTDebug("Sending score ...");
    BTScore *score = (BTScore *) packet->data;
    sendScore(score);
    break;
  }

  case BT_GAME_OVER: {
    if((err = pbuf_.sendpacket(BT_DEAD)) < 0) {
      cerr << "BattleTris: " << StreamSocketErrMsg(err) << endl;
      perror("BattleTris");
    }
    gameOver();
    break;
  }

  case BT_END_BAZ: {
    BTDebug("Sending BT_END_BAZ ...");
    if((err = pbuf_.sendpacket(BT_END_BAZ)) < 0)
      fatalErr(err);
    break;
  }

  case BT_PAUSE: {
    if((err = pbuf_.sendpacket(BT_PAUSE)) < 0) {
      cerr << "BattleTris: " << StreamSocketErrMsg(err) << endl;
      perror("BattleTris");
    }
    break;
  }

  default:
    break;
  }

  pass(packet);
}

void BTCommManager::receiveFromLocal(BTRingPacket *packet)
{
  // Since the other comm manager might buffer the packet and
  // later delete it, we have to create a _new_ packet for
  // the other comm manager (tpack).  Passing packet could lead
  // to a seg fault if it is deleted and then passed around the
  // local ring.

  switch(packet->token) {

  case BT_WPN_LAUNCH: {
    BTWeapon *wpn = (BTWeapon *) packet->data;
    BTDebug("Launching weapon " << wpn->token_);
    BTRingPacket *tpack = new BTRingPacket;
    tpack->token = BT_WPN_ON;
    tpack->data = (void *) wpn;
    cain_->receiveFromSibling(tpack);
    break;
  }

  case BT_CONDOR_OFF: {
    BTDebug("Turning off Condor");
    BTRingPacket *tpack = new BTRingPacket;
    tpack->token = packet->token;
    tpack->data = packet->data;
    cain_->receiveFromSibling(tpack);
    break;
  }

  case BT_SCORE: {
    BTDebug("Sending score ...");
    BTRingPacket *tpack = new BTRingPacket;
    tpack->token = packet->token;
    tpack->data = packet->data;
    cain_->receiveFromSibling(tpack);
    break;
  }

  case BT_GAME_OVER: {
    BTRingPacket *tpack = new BTRingPacket;
    tpack->token = BT_DEAD;
    cain_->receiveFromSibling(tpack);
    gameOver();
    break;
  }

  case BT_END_BAZ: {
    BTDebug("Sending BT_END_BAZ...");
    BTRingPacket *tpack = new BTRingPacket;
    tpack->token = BT_END_BAZ;
    cain_->receiveFromSibling(tpack);
    break;
  }

  case BT_PAUSE: {
    BTRingPacket *tpack = new BTRingPacket;
    tpack->token = BT_PAUSE;
    cain_->receiveFromSibling(tpack);
    break;
  }

  default:
    break;
  }

  pass(packet);
}

void BTCommManager::receiveFromSibling(BTRingPacket *packet)
{
  if(!(cain_ || sock_))
    return;

  switch(packet->token) {

  case BT_WPN_ON: {
    BTWeapon *wpn = (BTWeapon *) packet->data;
    BTDebug("received weapon " << wpn->token_);
    weapq_.insert_after_tail(wpn->token_);
    break;
  }
      
  case BT_CONDOR_OFF: {
    BTDebug("received BT_CONDOR_OFF");
    send(BT_CONDOR_OFF, 0);
    break;
  }

  case BT_START: {
    send(BT_START, packet);
    break;
  }
      
  case BT_SCORE: {
    BTDebug("received opponent score");
    send(BT_OP_SCORE, (BTScore *) packet->data);
    break;
  }
      
  case BT_BOARD: {
    BTBoard *board = (BTBoard *) packet->data;
    send(BT_BOARD, (void *) board);
    break;
  }

  case BT_ARSENAL: {
    BTArsenal *arsenal = (BTArsenal *) packet->data;
    send(BT_ARSENAL, (void *) arsenal);
    break;
  }
      
  case BT_END_BAZ: {
    send(BT_END_BAZ, 0);
    break;
  }

  case BT_DEAD: {
    gameOver();
    send(BT_DEAD, 0);
    break;
  }
      
  case BT_PAUSE: {
    send(BT_PAUSE, 0);
    break;
  }

  default:
    break;
  }

  delete packet;
}

void BTCommManager::gameCB(void)
{
  if(!(cain_ || sock_))
    return;

  short err;

  if((err = pbuf_.recvpacket()) < 0) {
    fatalErr(err);
    return;
  }

  switch(pbuf_.datatype()) {

  case BT_SCORE: {
    BTDebug("BT_SCORE received");
    recvScore();
    break;
  }

  case BT_WPN_ON: {
    BTDebug("BT_WPN_ON received");
    recvWeapon();
    break;
  }

  case BT_BOARD: {
    BTDebug("BT_BOARD received");
    recvBoard();
    break;
  }

  case BT_ARSENAL: {
    BTDebug("BT_ARSENAL received");
    recvArsenal();
    break;
  }

  case BT_DEAD: {
    BTDebug("BT_DEAD received");
    gameOver();
    startup_->won();
    send(BT_DEAD, 0);
    break;
  }

  case BT_END_BAZ: {
    BTDebug("BT_END_BAZ received");
    send (BT_END_BAZ, 0);
    break;
  }

  case BT_PAUSE: {
    send(BT_PAUSE, 0);
    break;
  }

  case BT_ERR: {
    BTDebug("BT_ERR received");
    BTMessageDlog errMsg(widget_, "Opponent aborted game or crashed.");
    send(BT_ERR, 0);
    startup_->error();
    gameOver();
  }

  }	
}

void BTCommManager::sendScore(BTScore *score)
{
  if(!(cain_ || sock_))
    return;

  char buf[sizeof(BTScore)];
  short err;

  score->writebuf(buf);

  if((err = pbuf_.sendpacket(BT_SCORE, sizeof(buf), buf)) < 0)
    fatalErr(err);
}

void BTCommManager::sendWeapon(BTWeapon *weapon)
{
  unsigned short wpn = weapon->token_;
  short err;

  wpn = htons(wpn);

  if((err = pbuf_.sendpacket(BT_WPN_ON, sizeof(wpn), (char *) &wpn)) < 0)
    fatalErr(err);
}

void
BTCommManager::sendBoard(BTBoard *board)
{
	char buf[(BT_BOARD_HGT * BT_BOARD_WTH * sizeof (unsigned long)) +
	    (3 * sizeof(unsigned short))];
	char *bufptr = buf;
	unsigned short ts;
	unsigned long tl;
	short err;
	int i, size;

	if (!(cain_ || sock_))
		return;

	if (cain_) {
		BTRingPacket *tpack = new BTRingPacket;
		tpack->token = BT_BOARD;
		tpack->data = (void *) board;
		cain_->receiveFromSibling(tpack);
		return;
	}

	BTNET_PUTSHORT(bufptr, ts, (unsigned short) board->motivation_);
	BTNET_PUTSHORT(bufptr, ts, (unsigned short) board->height_);
	BTNET_PUTSHORT(bufptr, ts, (unsigned short) board->width_);

	for (i = 0, size = board->rep_.size(); i < size; i++) {
		BTNET_PUTLONG(bufptr, tl, (unsigned long)board->rep_[i]);
	}

	if ((err = pbuf_.sendpacket(BT_BOARD, sizeof(buf), buf)) < 0)
		fatalErr(err);
}

void BTCommManager::sendArsenal(BTArsenal *arsenal)
{
  if(!(cain_ || sock_))
    return;

  if(cain_) {
    BTArsenal *new_arsenal = new BTArsenal;
    memcpy((void *) new_arsenal, (void *) arsenal, sizeof(BTArsenal));

    BTRingPacket *tpack = new BTRingPacket;
    tpack->token = BT_ARSENAL;
    tpack->data = (void *) new_arsenal;

    cain_->receiveFromSibling(tpack);

    return;
  }

  int arslen = BT_ARSENAL_SIZE;
  int buflen = sizeof(unsigned short) + (sizeof(unsigned short) * arslen * 2);
  char *buf = new char[buflen];
  char *bufptr = buf;

  unsigned short ts;
  short err;

  BTNET_PUTSHORT(bufptr, ts, (unsigned short) arslen);

  for(int i = 0; i < arslen; i++) {
    if(arsenal->rep_[i]) {
      BTNET_PUTSHORT(bufptr, ts, (unsigned short) arsenal->rep_[i]->token_);
    } else {
      BTNET_PUTSHORT(bufptr, ts, (unsigned short) BT_NO_WPN);
    }

    BTNET_PUTSHORT(bufptr, ts, (unsigned short) arsenal->quantity_[i]);
  }

  if((err = pbuf_.sendpacket(BT_ARSENAL, buflen, buf)) < 0)
    fatalErr(err);

  delete [] buf;

}

void BTCommManager::recvScore()
{
  BTScore *score = new BTScore;
  score->readbuf(pbuf_.databuf());
  send(BT_OP_SCORE, (void *) score);
  delete score;
}

void BTCommManager::recvWeapon()
{
  unsigned short ts = *((unsigned short *) pbuf_.databuf());
  BTWeaponToken token = (BTWeaponToken) ntohs(ts);
  weapq_.insert_after_tail(token);
}

void
BTCommManager::recvBoard()
{
	BTBoard *board = new BTBoard;
	char *bufptr = pbuf_.databuf();
	unsigned short ts;
	unsigned long tl;
	int motivation, i;

	BTNET_GETSHORT(bufptr, ts, motivation);
	BTNET_GETSHORT(bufptr, ts, board->height_);
	BTNET_GETSHORT(bufptr, ts, board->width_);

	board->motivation_ = (BTWeaponToken) motivation;
	int boardlen = board->height_ * board->width_;

	board->rep_.resize(boardlen);

	for (i = 0; i < boardlen; i++) {
		BTNET_GETLONG(bufptr, tl, board->rep_[i]);
	}

	if (board->motivation_ == BT_SWAP) {
		board_buf_ = board;
	} else {
		send(BT_BOARD, (void *)board);
		delete board;
	}
}

void BTCommManager::recvArsenal()
{
  BTArsenal *arsenal = new BTArsenal;
  char *bufptr = pbuf_.databuf();

  unsigned short arslen, wpn, ts;

  BTNET_GETSHORT(bufptr, ts, arslen);

  for(int i = 0; i < arslen; i++) {
    BTNET_GETSHORT(bufptr, ts, wpn);
    BTNET_GETSHORT(bufptr, ts, arsenal->quantity_[i]);

    if(wpn == BT_NO_WPN)
      arsenal->rep_[i] = 0;
    else
      arsenal->rep_[i] = (*pimp_)[wpn];
  }

  send(BT_ARSENAL, (void *) arsenal);
  // other ring nodes will take care of disposing of this arsenal
}

BTCommManager::~BTCommManager()
{
  if(sock_) {
    pbuf_.sendpacket(BT_ERR);
    delete sock_;
    sock_ = 0;
  }

  delete gamecb_;
}

int BTCommManager::startGame(StreamSocket *sock, char *opponentName)
{
  timeval timeout;
  timeout.tv_sec = BTCOMM_START_TIMEOUT;
  timeout.tv_usec = 0;

  pbuf_.socket(sock);
  sock_ = sock;
  cain_ = 0;

  short err;

  if(sock_->ready(timeout)) {
    if((err = pbuf_.recvpacket()) < 0) {
      fatalErr(err);
      return 0;
    }

    if(pbuf_.datatype() == BT_START) {
      sock_->installCB(gamecb_, SOCKET_CB_READ);
      send(BT_START, opponentName);
      return 1;
    } else if(pbuf_.datatype() == BT_ERR) {
      BTMessageDlog errMsg(widget_, "Opponent aborted game or crashed.");
      startup_->error();
      gameOver();
      return 0;
    } else {
      BTMessageDlog errMsg(widget_, "Invalid data received from opponent.");
      startup_->error();
      gameOver();
      return 0;
    }
  }

  BTMessageDlog errMsg(widget_, "Timed out waiting for opponent to start.");
  startup_->error();
  gameOver();
  return 0;
}

int BTCommManager::startGame(BTCommManager *sibling)
{
  cain_ = sibling;
  able_ = 0;

  sibling->cain_ = this;
  sibling->able_ = 1;

  if ( cain_->computer_ )
    send(BT_START, (void *) cain_->computer_->name());
  else
    send(BT_START, (void *) BTCOMM_COMPUTER_NAME);

  BTRingPacket *tpack = new BTRingPacket;
  tpack->token = BT_START;
  cain_->receiveFromSibling(tpack);

  return 1;
}

void BTCommManager::gameOver(void)
{
  if(sock_) {
    sock_->removeCB(SOCKET_CB_READ);
    sock_ = 0; // BTNetManager has a copy of this pointer and deletes it
  }

  if(cain_) {
    // We don\'t ever delete Ernie -- just use this pointer as a flag to
    // indicate if we\'re playing the computer or not
    cain_ = 0;
  }

  BTWeaponToken cruft;
  while(weapq_.remove_head(cruft));

  // Only set the game over callback to be called once

  if(!able_)
    DISPLAY->addTimeout(5000, BTStartup::gameOverTimeOut_CB, startup_);
}

void BTCommManager::flushWeapons(void)
{
  BTWeaponToken token;
  BTWeapon *weapon;

  while(weapq_.remove_head(token)) {
    weapon = new BTWeapon(token);
    send(BT_WPN_ON, (*pimp_)[token]);
    delete weapon;
  }

  if(board_buf_) {
    send(BT_BOARD, (void *) board_buf_);
    delete board_buf_;
    board_buf_ = 0;
  }
}

void BTCommManager::fatalErr(short err)
{
  cerr << "BattleTris: " << StreamSocketErrMsg(err) << endl;
  perror("BattleTris");
  BTMessageDlog errMsg(widget_, "Sorry, a network error occurred.");
  send(BT_ERR, 0);
  startup_->error();
  gameOver();
}
