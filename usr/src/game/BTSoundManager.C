/****************************************************************/
/*    NAME: Bryan Cantrill                                      */
/*    ACCT: bmc                                                 */
/*    FILE: BTSound.C                                           */
/*    ASSN:                                                     */
/*    DATE: Sun Feb 20 15:19:33 1994                            */
/****************************************************************/

#include "BTConfig.H"

#if HAVE_UNISTD_H
#include <unistd.h>
#endif

#if STDC_HEADERS
# include <stdlib.h>
#endif

// releng note -- this needs to be autoconf'ed --mws
#include <dirent.h>

#include <time.h>

#include "BattleTris.H"

#include "BTDebug.H"
#include "BTWeapon.H"
#include "BTDirs.H"
#include "BTSoundManager.H"

void
BTSoundManager::read_dir(const char *dir_name, Block<BTSoundFile *> *block)
{
	DIR *dir;
	struct dirent *dirent;
	char sound_dir[BT_MAX_SND_FILENAME];
	char name[BT_MAX_SND_FILENAME];

	strcpy(sound_dir, BT_SOUND_DIR);

	if ((dir = opendir(strcat(sound_dir, dir_name))) != NULL) {
		while ((dirent = readdir(dir)) != NULL) {
			if (strstr(dirent->d_name, ".au") == 0)
				continue;
			block->resize(block->size() + 1);
			strcpy(name, dir_name);
			strcat(name, dirent->d_name);
			block->operator[](block->size() - 1) =
			    new BTSoundFile(name);
  		}
		closedir (dir);
	}
  
	if(g_resources.r_rated == True &&
	    ((dir = opendir (strcat(sound_dir, ".r_rated"))) != NULL)) {
		char r_rated_name[BT_MAX_SND_FILENAME];

		strcpy(r_rated_name, dir_name);
		strcat(r_rated_name, ".r_rated/");
    
		while ((dirent = readdir (dir)) != NULL) {
			if (strstr(dirent->d_name, ".au") == 0)
				continue;
			block->resize(block->size() + 1);
			strcpy(name, r_rated_name);
			strcat(name, dirent->d_name);
			block->operator[](block->size() - 1) =
			    new BTSoundFile(name);
		}
		closedir (dir);
	}

	/*
	 * Mark all sounds as unplayed
	 */
	for (int j = 0 ; j < block->size(); j++)
		(block->operator[](j))->used_ = 0;
}

void BTSoundManager::play_random (Block<BTSoundFile *> *block) {
	if(block->size() <= 0)
		return;

	int i;

	// First make sure there is an unused entry
	if (block->operator[](0)->used_ == block->size()-1) {
		for (i = 0; i < block->size(); i++)
			(block->operator[](i))->used_ = 0;
	}
	for (;;) {
		i = (int) (drand48() * (block->size())) % block->size();
		if (i && (block->operator[](i)->used_ == 0))
			break;
	}
	block->operator[](i)->used_++;
	block->operator[](0)->used_++;
	*dev_ << (block->operator[](i))->name_;
}


BTSoundManager::BTSoundManager (DevAudio *dev) 
	: BTRingNode(), dev_ (dev), bad_move_ (0) {

	paused_ = 0;
	happy_ = 0;

	int i;
	// First read in all of the idiot sounds

	read_dir ("idiot/", &bad_move_);
	read_dir ("welcome/", &welcome_);
	read_dir ("start/", &start_);
	read_dir ("near_death/", &near_death_);
	read_dir ("tetris/", &tetris_);
	read_dir ("survived/", &survived_);
	read_dir ("won/", &won_);
	read_dir ("lost/", &lost_);
	read_dir ("launched/", &launched_);
	read_dir ("launched/wimpy/", &launch_wimpy_);

}  

void BTSoundManager::receive (BTRingPacket *packet) {

	if (dev_)
		switch (packet->token) {
		case BT_SCORE: {
			break;
		}
		case BT_LINE: {
			if (((BTLine *) packet->data)->inc() >= 4 
			    && ((BTLine *) packet->data)->inc() < 8) {
				play_random (&tetris_);
			}
			if (*((BTLine *) packet->data) == 8)
				*dev_ << "misc/sally.au";
			if ((*((BTLine *) packet->data) == 1)) {
				if (happy_) 
					*dev_ << "misc/im_so_happy.au";
				happy_ = 0;
			}
			break;
		}
		case BT_FUNDS: {
			if (*((short *) packet->data) >= BT_HAPPY_VAL)
				happy_ = 1;
			else
				happy_ = 0;
			break;
		}
		case BT_IDIOT: {
			switch (*((short *) packet->data)) {
			case BT_BAD_MOVE: {
				play_random (&bad_move_);
				break;
			}
			case BT_NEAR_DEATH: {
				if (rand() % 4 == 0) 
					play_random (&near_death_);
				break;
			}
			case BT_MISSED_SMILEY: {
				*dev_ << "misc/doh.au";
				break;
			}
			}
			break;
		}
		case BT_DEAD: {
			play_random (&won_);
			break;
		}
		case BT_GAME_OVER:
			if (!packet->data)
				play_random (&lost_);
			break;

		case BT_AIRSLIDE:
			*dev_ << "tetris/hoooah.au";
			break;

		case BT_PAUSE: {
			if (!paused_) {
				*dev_ << "misc/freeze_program.au";
				paused_ = 1;
			} else paused_ = 0;
			break;
		}  
		case BT_WPN_ON: {
			BTWeapon *wpn = (BTWeapon *) packet->data;
			switch (wpn->token()) {
			case BT_REAGAN: 
				*dev_ << "weapons/flush.au";
				break;
			case BT_FEARED_WEIRD: 
				*dev_ << "weapons/sub_dive_horn.au";
				break;
			case BT_BUG:
			case BT_PIECE_IT: 
				*dev_ << "weapons/Bottle.au";
				break;
			case BT_FALL_OUT:
				*dev_ << "weapons/swish.au";
				break;
			case BT_HATTER:
				*dev_ << "weapons/haveyou_gone_mad.au";
				break;
			case BT_KEATING:
				*dev_ << "weapons/Got_Your_Nose.au";
				break;
			case BT_SPEEDY:
				*dev_ << "weapons/MeepMeep.au";
				break;
			case BT_UPBYSIDE:
				*dev_ << "weapons/World_Gone_TopsyTurvy.au";
				break;
			case BT_GIMP:
				if(g_resources.r_rated)
					*dev_ << "misc/.r_rated/gimp.au";
				else
					*dev_ << "weapons/cmon.au";
				break;
			case BT_SUSAN:
				*dev_ << "weapons/give_to_me.au";
				break;
			case BT_CONDOR:
			case BT_AMES:
			case BT_ACE:
				//	Nay, bmc
				//        *dev_ << "weapons/Sonar.au";
				break;
			case BT_BLIND:
				*dev_ << "weapons/Explosion-2.au";
				break;
			case BT_RISE_UP:
				*dev_ << "weapons/jeffersons.au";
				break;
			case BT_FORCE:
				*dev_ << "weapons/force.au";
				break;
			case BT_FOUR_BY_FOUR:
				*dev_ << "weapons/abandon-ship.au";
				break;
			case BT_SLICK:
				*dev_ << "weapons/clinton.au";
				break;
			case BT_TWILIGHT:
				*dev_ << "weapons/TwilightZone.au";
				break;
			default: break;
			}
			break;
		}
		case BT_START:
			break;

		case BT_WPN_OFF: {
			BTWeapon *wpn = (BTWeapon *) packet->data;
			switch (wpn->token()) {
			case BT_FOUR_BY_FOUR:
			case BT_FEARED_WEIRD:
			case BT_SLICK:
			case BT_HATTER:
				play_random (&survived_);
				break;
			default: break;
			}
			break;
		}
		case BT_WPN_LAUNCH: {
			BTWeapon *wpn = (BTWeapon *) packet->data;
			int sound_no = 1;
			switch(wpn->token()) {
			case BT_GIMP:
				*dev_ << "weapons/gimp.au";
				sound_no--;
				break;
			default:
				break;
			}
			if ( sound_no == 0 )
				break;
			if (wpn->price_ <= 100) {
				play_random (&launch_wimpy_);
				break;
			}
			if (wpn->price_ > 400) {
				play_random (&launched_);
				break;
			}
		}
		default: break;
		}
	pass (packet);
}

void BTSoundManager::welcome() {
	int i;
	time_t curtime;

	time(&curtime);
	srand48(curtime + getpid());

	for(i = (curtime % 100); i; i--)
		lrand48();

	if(dev_ && welcome_.size() > 0)
		*dev_ << welcome_[i = lrand48() % welcome_.size()]->name_;
}

void BTSoundManager::playJeopardy() {
	if (dev_)
		*dev_ << "misc/jeopardy.au";
}

void BTSoundManager::start() {
	if (dev_) 
		play_random (&start_);
}

void BTSoundManager::cleanBlock( Block<BTSoundFile *> *block ) {
	for ( int j = 0 ; j < block->size() ; j++ )
		delete block->operator[](j);
}

BTSoundManager::~BTSoundManager() {
	cleanBlock(&bad_move_);
	cleanBlock(&welcome_);
	cleanBlock(&start_);
	cleanBlock(&near_death_);
	cleanBlock(&tetris_);
	cleanBlock(&survived_);
	cleanBlock(&won_);
	cleanBlock(&lost_);
	cleanBlock(&launched_);
	cleanBlock(&launch_wimpy_);
}
