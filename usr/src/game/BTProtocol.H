/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: BTProtocol.H                                        */
/*    DATE: Sun Feb  6 02:07:08 1994                            */
/****************************************************************/

#ifndef BTPROTOCOL_H
#define BTPROTOCOL_H

#define BT_SERVER_HOST	"poptart.eng.sun.com"
#define BT_SERVER_PORT	4404

enum BTToken {
  BT_NULL = 0,    // Null packet indicates that connection is over
  BT_ERR,         // A network error has occurred
  
  BT_SCORE = 10,  // Next token is a short indicating the change in score
  BT_OP_SCORE,    // Next token is a short indicating change in op's score
  BT_LINE,        // Represents an increase in lines
  BT_BOARD,       // Represents an increase in opponent\'s lines 
  BT_ARSENAL,     // Subsequent information represents opponents arsenal
  BT_FUNDS,       // Next token is short for increase in funds
  BT_WPN_ON,      // Next token is a BTWeaponToken indicating the weapon
  BT_WPN_LAUNCH,  // Next token is weapon to be launched.
  BT_WPN_OFF,     // Next token is a BTWeaponToken indicating the weapon
  BT_DEAD,        // Opponent has died.  Yer huge.
  BT_START_BAZ,   // Opponent has entered the weapons bazaar.
  BT_END_BAZ,     // Opponent has finished making selection
  BT_GAME_OVER,   // Your game\'s over.  You suck.
  BT_AIRSLIDE,    // You made an airlide.  You rock.
  BT_LAWYER,      // Lawyers' D.-induced rise-up.

  BT_PING = 30,   // An opponent is requesting your current status
  BT_BUSY,        // Currently busy--data is the login of an opponent
  BT_CHALL,       // Challenge--data is the login of an opponent
  BT_ACCPT,       // The opponent has accepted your challenge
  BT_DENY,        // The opponent has wimped out and doesn't want to play
  BT_START,       // Ride for battle!

  BT_PAUSE = 50,  // Opponent has paused the game
  BT_IDIOT,       // Your opponent is an idiot

  BT_CONDOR_OFF,  // Hacked up swill 6 hrs before due date

  // BattleTris Server Daemon <=> Slave Daemon Protocol

  BT_LOCAL = 60,  // Local client connection request
  BT_REMOTE,      // Remote client connection request
  BT_COOKIE_GOOD, // Magic cookie is tasty
  BT_COOKIE_BAD,  // "Magic" cookie is cheap Ratty imitation and is rejected
  BT_ACCEPTED,    // btserverd accepts the new client
  BT_REJECTED,    // btserverd says go get a life and find some friends, gimp
  BT_OBEY_ME,     // btserverd asks that btslaved bow down and obey
  BT_I_OBEY,      // btslaved responds that it will do btserverd\'s bidding
  BT_NEWCLIENT,   // btserverd asks that btslaved accept a new client
  BT_CLIENTOK,    // btslaved has accepted the client
  BT_CLIENTBAD,   // btslaved tells btserverd to bother some other slave
  BT_DISCONNECT,  // BattleTris client indicates that it is going to disconnect
  BT_HARIKARI,    // btserverd tells btslaved to commit ritual suicide

  // BattleTris Slave Daemon <=> Client Protocol

  BT_QUER_COOKIE, // Server request cookie from client
  BT_QUER_CONN,   // Client requests connection; data is BTNetworkEntry
  BT_QUER_NETDB,  // Client requests network database
  BT_QUER_PLYDB,  // Client requests player database
  BT_QUER_VERIFY, // Client requests verification of network entry; data is key
  BT_QUER_UPDATE, // Client requests a change in its network status
  BT_QUER_RESULT, // Client requests statistics be recorded; data is BTGameStats

  BT_RESP_COOKIE, // Data is BT_COOKIELEN bytes of encrypted cookie
  BT_RESP_VERIFY, // Data is unsigned short indicated whether entry was valid
  BT_RESP_DBLEN,  // Data is unsigned long indicated number of records
  BT_RESP_NETDB,  // Data is buffer of BTNetworkEntry records
  BT_RESP_PLYDB   // Data is buffer of BTPlayer records
};

enum BTWeaponToken {
  BT_FEARED_WEIRD = 0,	// The dreaded feared weird
  BT_FOUR_BY_FOUR,      // The even-more dreaded four-by-four
  BT_HATTER,
  BT_UPBYSIDE,
  BT_FALL_OUT,
  BT_SWAP,
  BT_LAWYERS,
  BT_RISE_UP,
  BT_FLIP_OUT,
  BT_SPEEDY,
  BT_MISSING, // 10
  BT_PIECE_IT,
  BT_BLIND,
  BT_MONDALE,
  BT_KEATING,
  BT_CARTER,
  BT_REAGAN,
  BT_AMES,
  BT_ACE,
  BT_CONDOR,
  BT_NICE_DAY, // 20
  BT_SO_LONG,
  BT_NO_DICE,
  BT_BUG,
  BT_BOTTLE,
  BT_NO_SLIDE,
  BT_SUSAN,
  BT_MEADOW,
  BT_MIRROR,
  BT_TWILIGHT,
  BT_SLICK,
  BT_BROKEN,
  BT_FORCE,
  BT_GIMP,
  BT_MAX_WEAPONS,
  BT_NO_WPN
};

#endif
