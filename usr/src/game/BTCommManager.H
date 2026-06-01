/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: BTCommManager.H                                     */
/*    DATE: Wed Apr 13 16:55:05 1994                            */
/****************************************************************/

#ifndef BTCOMMMANAGER_H
#define BTCOMMMANAGER_H

#include "BTConfig.H"
#include "List.H"
#include "BTRingNode.H"
#include "BTWeapon.H"
#include "BTScore.H"
#include "BTBoard.H"
#include "BTArsenal.H"
#include "BTPimp.H"
#include "StreamSocket.H"
#include "PacketBuffer.H"

class SocketCB;
class BTStartup;
class BTComputer;
class BTWidget;

class BTCommManager : public BTRingNode {

friend class BTGame;
friend class BTNetManager;

private:
  StreamSocket *sock_;
  PacketBuffer pbuf_;
  SocketCB *gamecb_;
  BTWidget *widget_;
  List<BTWeaponToken> weapq_;
  BTBoard *board_buf_;
  BTPimp *pimp_;	// The street-walkin\' pimp
  BTCommManager *cain_;	// The evil brother CommManager
  int able_;
  BTStartup *startup_;
  BTComputer *computer_;

private:
  static void gameCallback(Socket *sock, void *client_data) {
    ((BTCommManager *) client_data)->gameCB();
  }

  void fatalErr(short err);
  void gameCB();

  void recvScore();
  void recvWeapon();
  void recvBoard();
  void recvArsenal();

public:
  void sendScore(BTScore *score);
  void sendWeapon(BTWeapon *weapon);
  void sendBoard(BTBoard *board);
  void sendArsenal(BTArsenal *arsenal);

  BTCommManager(BTWidget *widget, BTPimp *pimp, 
		BTStartup *startup);

  ~BTCommManager();

  int startGame(StreamSocket *sock, char *opponentName);
  int startGame(BTCommManager *sibling);

  void gameOver();
  void flushWeapons();

  void receive(BTRingPacket *packet);
  void receiveFromLocal(BTRingPacket *packet);
  void receiveFromSibling(BTRingPacket *packet);

  void setComputer(BTComputer *computer) { computer_ = computer; }
};

#endif

