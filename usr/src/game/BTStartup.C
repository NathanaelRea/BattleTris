/****************************************************************/
/*    FILE: BTStartup.C                                         */
/*    CREDO: No hack is too colossal                            */
/****************************************************************/

#include "BTConfig.H"
#include <sys/types.h>
#include <signal.h>
#include <unistd.h>
#include "BattleTris.H"

#include "BTDirs.H"
#include "BTComputer.H"
#include "BTStartup.H"
#include "BTPixmap.H"
#include "BTCommManager.H"
#include "BTNetManager.H"
#include "DevAudio.H"
#include "PPMReader.H"
#include "BTGame.H"
#include "BTSoundManager.H"
#include "BTPimp.H"
#include "BTWidget.H"
#include "BTPushButtonWidget.H"
#include "BTFormWidget.H"
#include "BTLabelWidget.H"
#include "BTDrawingAreaWidget.H"
#include "BTDisplay.H"
#include "BTPlayer.H"
#include "BTNetworkEntry.H"

#define BT_STARTUP_FRAC_BASE 800

#define XFIX BT_STARTUP_FRAC_BASE / BT_STARTUP_WIDTH
#define YFIX BT_STARTUP_FRAC_BASE / BT_STARTUP_HEIGHT

#define BT_STARTUP_X 20
#define BT_STARTUP_Y 20

#define BT_STARTUP_DRAWING_AREA_X1 0 * XFIX
#define BT_STARTUP_DRAWING_AREA_Y1 0 * YFIX
#define BT_STARTUP_DRAWING_AREA_X2 580 * XFIX
#define BT_STARTUP_DRAWING_AREA_Y2 130 * YFIX

#define BT_STARTUP_CHALLENGE_BUTTON_X1 150 * XFIX
#define BT_STARTUP_CHALLENGE_BUTTON_Y1 475 * YFIX
#define BT_STARTUP_CHALLENGE_BUTTON_X2 300 * XFIX
#define BT_STARTUP_CHALLENGE_BUTTON_Y2 525 * YFIX

#define BT_STARTUP_SLEEP_BUTTON_X1 340 * XFIX
#define BT_STARTUP_SLEEP_BUTTON_Y1 475 * YFIX
#define BT_STARTUP_SLEEP_BUTTON_X2 490 * XFIX
#define BT_STARTUP_SLEEP_BUTTON_Y2 525 * YFIX

#define BT_STARTUP_ABOUT_BUTTON_X1 110 * XFIX
#define BT_STARTUP_ABOUT_BUTTON_Y1 535 * YFIX
#define BT_STARTUP_ABOUT_BUTTON_X2 230 * XFIX
#define BT_STARTUP_ABOUT_BUTTON_Y2 585 * YFIX

#define BT_STARTUP_ROSTER_BUTTON_X1 260 * XFIX
#define BT_STARTUP_ROSTER_BUTTON_Y1 535 * YFIX
#define BT_STARTUP_ROSTER_BUTTON_X2 380 * XFIX
#define BT_STARTUP_ROSTER_BUTTON_Y2 585 * YFIX

#define BT_STARTUP_QUIT_BUTTON_X1 410 * XFIX
#define BT_STARTUP_QUIT_BUTTON_Y1 535 * YFIX
#define BT_STARTUP_QUIT_BUTTON_X2 530 * XFIX
#define BT_STARTUP_QUIT_BUTTON_Y2 585 * YFIX

// If you\'ve neglected challenges more than this number, you\'re outta here!
#define BT_MAX_CHAL_TIMEOUTS 1

#define BT_AVG_REJECTION_NO 3
#define BT_BMCCGHMWS_REJECTION_NO 20

BTStartup::BTStartup( BTWidget *parent )
: parent_(parent), app_(g_appctx),
  accepted_(-1), challengee_(0), challenge_(0), gimp_image_(0),
  playing_again_ (0), x_(BT_STARTUP_X), y_(BT_STARTUP_Y), location_(BT_STARTUP),
  computer_(0), dev_(0), chal_timeouts_(0)
{
	BTPixmap *bazaar, *icon, *startup_image, *chal, *sleep;

  srand(time(0) + getpid());

  if(g_resources.r_rated) {
    cout << "BattleTris: Undocumented Rated-R mode enabled" << endl;
    cout << "BattleTris: If terms like ``fat chick'' offend you, ctrl-C now ..." << endl;
  }
  
  if(g_resources.mute == False) {
    dev_ = new DevAudio(g_resources.headphones, BT_SOUND_DIR);
    
    if(!(*dev_)) {
      cout << "BattleTris: Could not open dev audio..." << endl;
      delete dev_;
      dev_ = 0;
    } 
  } else {
    dev_ = 0;
  }

  sound_manager_ = new BTSoundManager(dev_);

  if(g_resources.mute == False) {
    if(g_resources.a_team == False) {
      sound_manager_->welcome();
    } else {
      if(dev_)
        *dev_ << "welcome/a-team.au";
    }
  }

  // Create the pimp, which reads in the weapons database files, and
  // establish the necessary network connection with the BattleTris Server

  pimp_ = new BTPimp;

  if(!pimp_->load()) {
    cerr << "BattleTris: Failed to load weapons database files" << endl;
    cerr << "BattleTris: Fatal error occurred" << endl;
    bt_terminate(1);
  }

  comm_manager_ = new BTCommManager(parent_, pimp_, this);
  computer_comm_ = new BTCommManager(parent_, pimp_, this);
  net_manager_ = new BTNetManager(parent_, this, comm_manager_,
				  g_resources.serverHost, g_resources.serverPort);

  // Allocate the color palette and the PPM reader
  reader_ = new PPMReader(parent_->getWidget(), BT_ART_DIR);

  if(!(*reader_)) {
    cout << "BattleTris: Creating private colormap ..." << endl;
    DISPLAY->newPalette();

    delete reader_;
    reader_ = new PPMReader(parent_->getWidget(), BT_ART_DIR);

    if(!(*reader_)) {
      cerr << "BattleTris: Cannot allocate necessary colors" << endl;
      cerr << "BattleTris: Please close color-intensive apps" << endl;
      bt_terminate(1);
    }
  }

	cout << "BattleTris: Loading Images ..." << flush;
	int i = 0;

	bazaar = loadImage("btbazaar.ppm");
	icon = loadImage("btbiff1.ppm");
	startup_image = loadImage("btstartup2.ppm");
	chal = loadImage("btchalbiff.ppm");
	sleep = loadImage("btsleepbiff.ppm");
  
  	gimp_image_ = loadImage(g_resources.r_rated ?
	    "btgimp.ppm" : "btgimp2.ppm");
	gimp_image_->ref();
  
  cout << " Done.\nBattleTris: Loading BattleTris ..." << flush;

  bazaar_ = new BTBazaar(parent_, pimp_, comm_manager_, bazaar);
  cout << '.' << flush;

  game_ = new BTGame(parent_, sound_manager_, comm_manager_, pimp_, bazaar_, gimp_image_);
  cout << '.' << flush;
  
  about_box_ = new BTAbout(parent_, icon);
  cout << '.' << flush;

  challenge_screen_ = new BTChallenge(parent_, net_manager_, icon);
  cout << '.' << flush;

  roster_ = new BTRoster(parent_, net_manager_, icon);
  cout << '.' << flush;

  biff_ = new BTBiff(parent_, sleep, chal, dev_);
  cout << '.' << flush;

  biff_->sleep_daw_->addInputCallback(handleBiffClick_CB, this);
  biff_click_ = 0;

  cout << '.' << flush;

  about_box_->ok_button_->addActivateCallback(handleAboutOK_CB, this);
  roster_->done_button_.addActivateCallback(handleRosterDone_CB, this);
  challenge_screen_->cancel_button_.addActivateCallback(handleChalCancel_CB, this);
  challenge_screen_->computer_button_.addActivateCallback(handleComputer_CB, this );

  cout << '.' << flush;

  // Create the form widget for the main screen and the drawing area
  // which holds the BattleTris startup image

  form_ = new BTFormWidget(parent_, "BTStartup", BT_STARTUP_WIDTH,
			   BT_STARTUP_HEIGHT, BT_STARTUP_FRAC_BASE);

  drawing_area_ =
    new BTDrawingAreaWidget(form_, "drawing_area",
			    startup_image, startup_image->width_, 
			    startup_image->height_,
			    BT_STARTUP_DRAWING_AREA_X1,
			    BT_STARTUP_DRAWING_AREA_Y1);

  drawing_area_->manage();

  cout << '.' << flush;

  // Create the challenge dialog and add callbacks to the yes and no buttons

  if(g_resources.r_rated == False)
    chal_dialog_ = new BTChallengeDialog (parent_);
  else
    chal_dialog_ = new BTChallengeDialog (parent_);

  chal_dialog_->accept_->addActivateCallback(handleYes_CB, this );
  chal_dialog_->decline_->addActivateCallback(handleNo_CB, this );

  cout << '.' << flush;

  // Create and manage all of the button widgets on the main screen

  sleep_button_ =
    new BTPushButtonWidget(form_, "sleep_button", "Sleep");

  form_->placeChild( sleep_button_,
		     BT_STARTUP_SLEEP_BUTTON_X1, BT_STARTUP_SLEEP_BUTTON_Y1,
		     BT_STARTUP_SLEEP_BUTTON_X2, BT_STARTUP_SLEEP_BUTTON_Y2 );

  sleep_button_->manage();

  about_button_ =
    new BTPushButtonWidget(form_, "about_button", "About");

  form_->placeChild( about_button_,
		     BT_STARTUP_ABOUT_BUTTON_X1, BT_STARTUP_ABOUT_BUTTON_Y1,
		     BT_STARTUP_ABOUT_BUTTON_X2, BT_STARTUP_ABOUT_BUTTON_Y2 );

  about_button_->manage();

  challenge_button_ =
    new BTPushButtonWidget(form_, "challenge_button", "Challenge" );
  form_->placeChild( challenge_button_,
		     BT_STARTUP_CHALLENGE_BUTTON_X1, BT_STARTUP_CHALLENGE_BUTTON_Y1,
		     BT_STARTUP_CHALLENGE_BUTTON_X2, BT_STARTUP_CHALLENGE_BUTTON_Y2 );
  challenge_button_->manage();

  roster_button_ =
    new BTPushButtonWidget(form_, "roster_button", "Roster");
  form_->placeChild( roster_button_,
		     BT_STARTUP_ROSTER_BUTTON_X1, BT_STARTUP_ROSTER_BUTTON_Y1,
		     BT_STARTUP_ROSTER_BUTTON_X2, BT_STARTUP_ROSTER_BUTTON_Y2 );
  roster_button_->manage();

  quit_button_ =
    new BTPushButtonWidget(form_, "quit_button", "Quit",
			   BT_STARTUP_QUIT_BUTTON_X2-BT_STARTUP_QUIT_BUTTON_X1,
			   BT_STARTUP_QUIT_BUTTON_Y2-BT_STARTUP_QUIT_BUTTON_Y1);

  form_->placeChild(quit_button_,
		    BT_STARTUP_QUIT_BUTTON_X1, BT_STARTUP_QUIT_BUTTON_Y1,
		    BT_STARTUP_QUIT_BUTTON_X2, BT_STARTUP_QUIT_BUTTON_Y2 );
  quit_button_->manage();
  
  cout << '.' << flush;

  // Add callbacks to all of the buttons on the main screen

  quit_button_->addActivateCallback(handleQuit_CB, this);
  roster_button_->addActivateCallback(handleRoster_CB, this);
  challenge_button_->addActivateCallback(handleChallenge_CB, this);
  sleep_button_->addActivateCallback(handleSleep_CB, this);
  about_button_->addActivateCallback(handleAbout_CB, this);

  cout << " Done.\n" << flush;
}

BTPixmap *
BTStartup::loadImage(const char *name)
{
	PPMPixel *data;
	int width, height;
	XImage *image;
	BTPixmap *pixmap;

	if (!reader_->readPpmFile(name, data, width, height))
		bt_terminate(1);

	if ((image = reader_->createImage(data, width, height)) == NULL)
		bt_terminate(1);

	pixmap = new BTPixmap(image, width, height, 1);
	cout << '.' << flush;

	return (pixmap);
}

void BTStartup::show(int first_time)
{
  parent_->size( -1, -1, BT_STARTUP_WIDTH, BT_STARTUP_HEIGHT );
  if(!first_time) 
    parent_->map();
  form_->manage();
}

void BTStartup::hide()
{
  parent_->unmap();
  form_->unmanage();
}

void BTStartup::childDied() {
  if(sound_manager_)
    sound_manager_->setdev((DevAudio *) 0);
}

void BTStartup::handleBiffClick() {
  if ( biff_->sleep_daw_->button_released_ ) {
    biff_->hide();
    if(challenge_) {
      biff_->changeBiff();
      chal_dialog_->show();
      chal_dialog_->map(); // XMapWindow(XtDisplay (parent_), XtWindow(chal_dialog_->getWidget())); 
    } else {
      location_ = BT_STARTUP;
      show(0);
    }  
  }
}

void BTStartup::showGame() {
  if (location_ == BT_STARTUP) 
    form_->unmanage();
  if (location_ == BT_ROSTER) 
    roster_->hide();
  if (location_ == BT_CHAL) {
    challenge_screen_->hide();
    form_->unmanage();
  }
  parent_->size(-1, -1, BT_GAME_WIDTH + 1, BT_GAME_HEIGHT + 1);
  parent_->noResize();
}  

void BTStartup::showBazaarFixture() {
  showGame();
  game_->showBazaarFixture();
  location_ = BT_ERNIE;
}

void BTStartup::showReconFixture() {
  showGame();
  game_->showReconFixture();
  location_ = BT_ERNIE;
}

void BTStartup::hideGame() {
  game_->form_->unmanage();
  show(0);
}
  
void BTStartup::challengeTimeOut (unsigned long *) {
  challenge_ = 0;
  challengee_ = 0;
  accepted_ = 0;
  net_manager_->busy_ = 0;
  chal_dialog_->hide();
  if (location_ == BT_BIFF) {
    if (biff_->asleep()) {
      biff_->show();
    } else 
      biff_->changeBiff();
    if (++chal_timeouts_ > BT_MAX_CHAL_TIMEOUTS)
      bt_terminate(0);
  } else {
    if (location_ == BT_ERNIE) {
      game_->challenge(0);
      game_->unpause();
    }
    if (++chal_timeouts_ > BT_MAX_CHAL_TIMEOUTS)
      bt_terminate(0);
  }
}

void BTStartup::handleYes() {
  DISPLAY->removeTimeout( chal_time_ );

//  chal_dialog_->hide();

  // Reset the challenge timeout count
  chal_timeouts_ = 0;

  switch(location_) {  

  case BT_ROSTER: 
    roster_->hide();
    // Fall through

  case BT_BIFF:

    challenge_ = 0;
    location_ = BT_STARTUP;
    show(0);

    // Fall through....
  case BT_STARTUP: 
    form_->unmanage();
    parent_->size(-1, -1, BT_GAME_WIDTH, BT_GAME_HEIGHT);
    break;

  case BT_ERNIE:
    // Kill the game...
    game_->unpause();

    // Set a special flag to tell the game to not go away...
    playing_again_ = 1;

    game_->endGame();
    hideGame();
    game_->reset();

    // And set the values correctly...
    parent_->size(-1, -1, BT_GAME_WIDTH, BT_GAME_HEIGHT);
  default: break;
  }

  challengee_ = 1;
  accepted_ = 1;
  won_ = 0;
}

void BTStartup::handleNo () {
  DISPLAY->removeTimeout( chal_time_ );

  const char *key = net_manager_->entry_ ? net_manager_->entry_->userName_ : 0;
  if (!key || !(strcmp(key,"cgh") == 0 || strcmp(key,"bmc") == 0 ||
		strcmp(key,"mws") == 0))
    if (--rejections_) {
      cerr << "BattleTris: Come again?\007" << endl << flush;
      chal_dialog_->show();
      if (dev_) {
	int r = rand() % 7;
	switch( r ) {
	case 1:
	  *dev_ << "misc/soft_stool.au";
	  break;
	case 2:
	  *dev_ << "misc/nervous.au";
	  break;
	case 3:
	  *dev_ << "misc/letsgo.au";
	  break;
	case 4:
	  *dev_ << "misc/Homer_Scream.au";
	  break;
	case 5:
	  *dev_ << "misc/oj_married.au";
	  break;
	case 6:
	  *dev_ << "idiot/coverup1.au";
	  break;
	default:
	  *dev_ << "misc/failure.au";
	  break;
	}
      }
      chal_time_ = 
	DISPLAY->addTimeout(1000 * BTNETMGR_TIMEOUT, challengeTimeOut_CB, this);
      return;
/*
    cerr << "BattleTris: Thou shalt not reject the authors' challenges!"
	 << endl << flush;
    if (dev_) {
      *dev_ << "misc/soft_stool.au";
      sleep(2);
    }
    bt_terminate(0);
    */
    }

  if(dev_) {
    if(g_resources.r_rated)
      *dev_ << "misc/beastie_chico.au";
    else
      *dev_ << "misc/sorry.au";
  }

  if(location_ == BT_BIFF) 
    biff_->show();

  if (location_ == BT_ERNIE) {
    game_->unpause();
    game_->challenge (0);
  }

  challengee_ = 0;
  challenge_ = 0;
  accepted_ = 0;
}

BTStartup::~BTStartup()
{
  if(computer_)
    delete computer_;

  delete sleep_button_;
  delete about_button_;
  delete challenge_button_;
  delete roster_button_;
  delete quit_button_;
  delete drawing_area_;

  delete chal_dialog_;
  delete form_;

  delete biff_;
  delete roster_;
  delete challenge_screen_;
  delete about_box_;
  delete game_;

  delete bazaar_;

  if(gimp_image_->deref())
    delete gimp_image_;

  delete reader_;
  delete net_manager_;
  delete computer_comm_;
  delete comm_manager_;
  delete pimp_;
  delete sound_manager_;

  if(dev_) {
     pid_t audio_pid = dev_->slavePid();
     delete dev_;

     // Hell, let\'s not play any games here
     kill(audio_pid, 9);
  }
}

void BTStartup::gameOverTimeOut (unsigned long *) {
  if (playing_again_) {
    net_manager_->avail_ = 0;
    playing_again_ = 0;
    return;
  }

  hideGame();
  game_->reset();

  net_manager_->gameOver();

  if (won_ < 0) {
    won_ = 0;
  } else if (challengee_) {
    net_manager_->recordStats (won_, game_->stats_);
  }
  challengee_ = 0;
}

void BTStartup::handleSleep () {
  hide();
  challengee_ = 0;
  challenge_ = 0;
  biff_->show();
  location_ = BT_BIFF;
}

void BTStartup::handleAbout () {
  location_ = BT_ABOUT;
  about_box_->show();
}

void BTStartup::handleAboutOK () {
  if (about_box_->click_ == 2) {
    if(dev_) {
      if(about_box_->eggs_[BT_CHARLIE_EGG] && about_box_->eggs_[BT_MIKE_EGG]
        && about_box_->eggs_[BT_BRYAN_EGG]) {
        *dev_ << ".eggs/us.au";
          goto out;
      }
      if(about_box_->eggs_[BT_CHARLIE_EGG]) {
        *dev_ << ".eggs/charlie.au";
        goto out;
      }
      if (about_box_->eggs_[BT_MIKE_EGG]) {
        *dev_ << ".eggs/mike.au";
        goto out;
      }
      if (about_box_->eggs_[BT_BRYAN_EGG]) {
        *dev_ << ".eggs/bryan.au";
        goto out;
      }
      if (about_box_->eggs_[BT_LIBBY_EGG]) {
        *dev_ << ".eggs/libby.au";
        goto out;
      }
      if (about_box_->eggs_[BT_KEVIN_EGG]) {
        *dev_ << ".eggs/kevin.au";
        goto out;
      }
      *dev_ << "misc/cant_touch.au";
    }

    out:
    for (int i = 0; i < BT_MAX_EGGS; i++) 
      about_box_->eggs_[i] = 0;
    about_box_->click_ = 0;
    return;
  }

  location_ = BT_STARTUP;
  about_box_->hide();
}

void BTStartup::handleChallenge () {
  challengee_ = 0;
  won_ = 0;
  game_->computer_ = 0;

  location_ = BT_CHAL;

  parent_->size( -1, -1, BT_CHALLENGE_WIDTH, BT_CHALLENGE_HEIGHT );
  challenge_screen_->show();
/*
    // USED TO RUN IF BT_BUNNY SET: --mws
    XtVaSetValues (*parent_, XmNwidth, BT_GAME_WIDTH + 1,
		   XmNheight, BT_GAME_HEIGHT + 1, XmNx, 40, XmNy, 40, 0);
    form_->unmanage();
    game_->run();  
 */
}
      
void BTStartup::handleChalCancel () {
  parent_->size(-1, -1, BT_STARTUP_WIDTH, BT_STARTUP_HEIGHT);
  challenge_screen_->hide();
}

void
BTStartup::handleComputer()
{
	// Reset the challenge timeout count
	chal_timeouts_ = 0;

	if (dev_) {
		if (challenge_screen_->level_ == BTComputer::nLevels() - 1) {
			*dev_ << "misc/bionic.au";
		} else {
			*dev_ << "misc/knight1.au";
		}
	}

	if (!computer_)
		computer_ = new BTComputer(computer_comm_, game_, pimp_);

	computer_comm_->setComputer(computer_);
	computer_->reset(challenge_screen_->level_);
	game_->computer_ = computer_;
	net_manager_->setComputer(computer_comm_);
	net_manager_->challengeComputer(challenge_screen_->avail_);
	location_ = BT_ERNIE;
}

void BTStartup::handleQuit(  )
{
  hide();
  bt_terminate(0);
}

void BTStartup::handleRoster(  )
{
  location_ = BT_ROSTER;
  parent_->size( -1, -1, BT_ROSTER_WIDTH, BT_ROSTER_HEIGHT);
  challengee_ = 0; 	 
  roster_->show();

}

void BTStartup::handleRosterDone(  )
{
  roster_->hide();
  parent_->size( -1, -1, BT_STARTUP_WIDTH, BT_STARTUP_HEIGHT);
  challengee_ = 0;
  location_ = BT_STARTUP;
}

/*void BTStartup::challenge (char *challenger, char *node) {*/
void BTStartup::challenge( BTPlayer *player ) {

  unsigned short width;
  char msg[255];

  accepted_ = -1;
  challengee_ = 0;
  challenge_ = 1;

//  width = chal_dialog_->headline_->width();
  msg[0] = 0;

  if(g_resources.r_rated == False) {
    chal_dialog_->chal_smack_->setLabel("wants to mix it up.");
//    sprintf(msg, "wants to mix it up.", challenger, node);
  } else {
    chal_dialog_->chal_smack_->setLabel("wants a piece of yo' ass.");
//    sprintf(msg, "wants a piece of yo' ass.", challenger, node);
  }

  chal_dialog_->player(player);

  challenger_ = player;

  const char *key = player->key();

  if (strcmp(key,"cgh") == 0 || strcmp(key,"bmc") == 0 ||
      strcmp(key,"mws") == 0)
    rejections_ = rand() % BT_BMCCGHMWS_REJECTION_NO + 1;
  else
    rejections_ = rand() % BT_AVG_REJECTION_NO + 1;

/*
  chal_dialog_->headline_->setLabel(msg);
  chal_dialog_->headline_->size( -1, -1, width);
  width = chal_dialog_->headline_->width();
  */

  switch (location_) {
  case BT_BIFF: 
    biff_->changeBiff();
    if (dev_)
      *dev_ << "misc/play_a_game.au";
    else
      cout << "\007" << flush;
    break;

  case BT_ERNIE:
    // This is a little confusing...we need to first set the game's
    // challenge dialog to point to this chal_dialog...
    assert (game_);
    game_->challenge (chal_dialog_);
    break;

  default:
    if (dev_)
      *dev_ << "misc/play_a_game.au";
    else
      cout << "\007" << flush;
    chal_dialog_->show();
  }
  chal_time_ = 
    DISPLAY->addTimeout(1000 * BTNETMGR_TIMEOUT, challengeTimeOut_CB, this);
}
