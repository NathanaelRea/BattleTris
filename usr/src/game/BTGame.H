/****************************************************************/
/*    NAME: Bryan Cantrill                                      */
/*    ACCT: bmc                                                 */
/*    FILE: BTGame.H                                            */
/*    ASSN:                                                     */
/*    DATE: Mon Feb 21 18:30:41 1994                            */
/****************************************************************/

#ifndef BTGAME_H
#define BTGAME_H

#include "BTConfig.H"

#define BT_GAME_WIDTH 670
#define BT_GAME_HEIGHT 700
#define BT_GAME_SCORE_X 300
#define BT_GAME_SCORE_Y 30
#define BT_GAME_ARSENAL_X 300
#define BT_GAME_ARSENAL_Y 270

#define BT_GAME_FRAC_BASE 800

#include <Xm/MainW.h>
#include <Xm/DrawingA.h>

#include "BTConstants.H"
#include "BTProtocol.H"

#include <BTScrolledTextWidget.H>
#include <BTScrolledListWidget.H>
#include <BTPushButtonWidget.H>
#include <BTFormWidget.H>
#include <BTLabelWidget.H>
#include <BTDrawingAreaWidget.H>

#include "BTRingNode.H"
#include "BTTimeOut.H"

#include <stdlib.h>

#define BT_DROP_TIMEOUT         0
#define BT_SLIDE_TIMEOUT        1
#define BT_SLICK_TIMEOUT        2
#define BT_HATTER_TIMEOUT       3
#define BT_JEOPARDY_TIMEOUT     4
#define BT_MAX_GAME_TIMEOUT     5

#define XK_LATIN1
#include <X11/keysymdef.h>
#include "BTStopwatch.H"

class BTBazaar;
class BTPimp;
class BTPieceManager;
class BTScoreManager;
class BTSoundManager;
class BTCommManager;
class BTWeaponManager;
class BTBoardManager;
class BTRecon;
class BTStopwatch;
class BTGameStats;
class BTComputer;
class BTChallengeDialog;
class BTPixmap;
class BTPiece;

class BTGame : public BTRingNode {
friend class BTStartup;
friend class BTCommManager;
private:
	BTTimeOut timeout_[BT_MAX_GAME_TIMEOUT];

	int sliding_;
	int drop_;
	int old_lines_;		// opponent's line count, for Lawyers' Delite
	BTGameStats *stats_;
	BTPimp *pimp_;
	BTFormWidget *form_;
	BTDrawingAreaWidget *drawing_area_;
	BTWidget *parent_;
	int slick_dir_;
	int swapper_, paused_, started_;
	int op_done_baz_, done_baz_, in_baz_;
	int condor_on_;
	int base_drop_time_, fast_drop_time_;
	int *drop_time_, *slide_time_;
	int x_, y_;
	int def_x_, def_y_;
	int delta_y_;
	int left_x_, right_x_;
	char opponent_msg_[255];
	unsigned short old_width_, old_height_;
	short m;
	BTRecon *recon_;
	BTBoardManager *board_, *initial_board_;
	BTPieceManager *piece_manager_;
	BTScoreManager *score_manager_;
	BTSoundManager *sound_manager_;
	BTCommManager *comm_manager_;
	BTWeaponManager *weapon_manager_;
	BTStopwatch stopwatch_;
	BTLabelWidget *message_;
	BTBazaar *bazaar_;
	BTPiece *current_piece_;
	BTComputer *computer_;
	BTChallengeDialog *chal_dialog_;
	static void _keypress2_(char, void *);
	static void _pausekey2_(char, void *);

	BT_TIMEOUT_CALLBACK (BTGame, timeout);
	BT_TIMEOUT_CALLBACK (BTGame, slidetime);
	BT_TIMEOUT_CALLBACK (BTGame, hattertime);
	BT_TIMEOUT_CALLBACK (BTGame, slicktime);
	BT_TIMEOUT_CALLBACK (BTGame, jeotime);

	void exposeEvent ();
	void leaveBazaar();
	void runComputer();

	void addTimeOut (int id);
	void removeTimeOut (int id);
	void pauseTimeOut (int id);
	void unpauseTimeOut (int id);
	void expireTimeOut (int id);
	void clearTimeOut (int id);
	void removeAllTimeOuts();
	void pauseAllTimeOuts();
	void unpauseAllTimeOuts();

	BTPixmap *gimp_pixmap_;

public:
	BTGame(BTWidget *parent, BTSoundManager *sound_manager,
	    BTCommManager *, BTPimp *pimp, BTBazaar *bazaar,
	    BTPixmap *gimp = NULL);
	~BTGame();
	void receive(BTRingPacket *packet);
	void startGame();
	void endGame();
	void pause(int no_send = 0);
	void unpause();
	void keyPressed (char c);
	void drop();
	void startSlide();
	void lawyers(int nlines);
	void place(int force = 0);
	void beginDrop();
	void moveLeft();
	void moveRight();
	void condor();
	void reset();
	void cleanUp();
	void rotate();
	void challenge(BTChallengeDialog *chal ) { chal_dialog_ = chal; }
	void run() { sendPlusMe (BT_START, (void *)"error."); }
	int gimpMe( int, int );
	int isDropping() { return *drop_time_ == fast_drop_time_; }
	static void _gamecb2_(BTWidget *, void *d) {
		((BTGame *)d)->exposeEvent();
	}
	static void _bazcb2_(BTWidget *, void *);
};    

#endif
