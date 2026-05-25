/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: BTPlayer.C                                          */
/*    DATE: Thu Apr 28 02:15:27 1994                            */
/****************************************************************/

#include "BTConfig.H"

#if STDC_HEADERS
# include <stdlib.h>
# include <ctype.h>
#else
# define isprint(x) ((x > 31) && (x < 127))
#endif

#include <sys/stat.h>

#if HAVE_UNISTD_H
# include <unistd.h>
#endif


#include <fstream>
using namespace std;
#include <assert.h>
#include <stdio.h>
#include <math.h>
#include <pwd.h>
#include <string.h>
#include <strings.h>

#include "BTGameStats.H"
#include "BTNetwork.H"
#include "BTPlayerRecord.H"
#include "BTPlayer.H"
#include "BTDirs.H"
#include "BTDBErr.H"
#include "BTDB.H"

/* XXX Old ranking algorithm which sucks
static unsigned long
recompute_elo_rank(unsigned long oldrank, unsigned long opprank, int win)
{
  unsigned long grow = win ? 16 : -16;

#define elo_max(x, y) ((x) < (y) ? (y) : (x))
#define elo_min(x, y) ((x) < (y) ? (x) : (y))

  return elo_min(elo_max(((opprank - oldrank) / 25), -15), 15) + grow + oldrank;

#undef elo_max
#undef elo_min
} */

static unsigned long
recompute_elo_rank(unsigned long oldrank, unsigned long opprank, int win)
{
#define average_game_value 5
  unsigned long k; 
 
  if(win) {
    if((k = average_game_value * opprank / oldrank) == 0)
      return oldrank + 1;
    else
      return oldrank + k;
  } else {
    if((k = average_game_value * oldrank / opprank) >= oldrank)
      return 1;
    else
      return oldrank - k;
  }
#undef average_game_value
}

BTPlayer::BTPlayer(const char *name)
: BTDBRecord(name), rank_(BT_ELO_START), wins_(0), losses_(0), highScore_(0),
  highLines_(0), highFunds_(0), streak_(0), streakType_(BTSTREAK_EMPTY),
  fastestKill_(0), quickestDeath_(0), longestGame_(0),
  records_(0), rsize_(0)
{
  if(*name != '\0') {
    char namebuf[BTDBRECORD_KEYLEN + 1];
    struct passwd *pwentry;
    char *ptr;

    strcpy(namebuf, name);

    if((ptr = strchr(namebuf, '@')))
      *ptr = '\0';

    if((pwentry = getpwnam(namebuf))) {
      // Grab the user\'s real name from the gecos field
      bzero((char *) gecos_, sizeof(gecos_));
      strncpy(gecos_, pwentry->pw_gecos, BT_GECOSNAMELEN);
      
      // Here at Brown and many other sites, the gecos field is comma-
      // separated and contains other info such as office phone number,
      // so only grab the stuff up to the first comma

      if((ptr = strchr(gecos_, ',')))
	*ptr = '\0';
    }
  }

  size_ += BTPlayer::datalen();
  valid_ = 1;
}

BTPlayer::BTPlayer(const BTPlayer& other)
: BTDBRecord(other.key_), rank_(other.rank_), wins_(other.wins_),
  losses_(other.losses_), highScore_(other.highScore_),
  highLines_(other.highLines_), highFunds_(other.highFunds_),
  streak_(other.streak_), streakType_(other.streakType_),
  fastestKill_(other.fastestKill_), quickestDeath_(other.quickestDeath_),
  longestGame_(other.longestGame_)
{
  valid_ = other.valid_;
  size_ = other.size_;

  strncpy(gecos_, other.gecos_, BT_GECOSNAMELEN);
  rsize_ = other.rsize_;

  if(other.records_ != 0) {
    records_ = new BTPlayerRecord * [rsize_];

    for(short i = 0; i < rsize_; i++)
      records_[i] = new BTPlayerRecord(*(other.records_[i]));
  }
}

BTPlayer::~BTPlayer(void)
{
  if(records_) {
    for(short i = 0; i < rsize_; i++)
      if(records_[i] != 0)
        delete records_[i];
    delete [] records_;
  }
}

BTPlayer& BTPlayer::operator=(const BTPlayer& other)
{
  if(this == &other)
    return *this;

  strncpy(key_, other.key_, BTDBRECORD_KEYLEN);
  valid_ = other.valid_;
  size_ = other.size_;

  strncpy(gecos_, other.gecos_, BT_GECOSNAMELEN);

  wins_ = other.wins_;
  losses_ = other.losses_;
  highScore_ = other.highScore_;
  highLines_ = other.highLines_;
  highFunds_ = other.highFunds_;

  streak_ = other.streak_;
  streakType_ = other.streakType_;
  fastestKill_ = other.fastestKill_;
  quickestDeath_ = other.quickestDeath_;
  longestGame_ = other.longestGame_;

  rsize_ = other.rsize_;

  if(other.records_ != 0) {
    records_ = new BTPlayerRecord * [rsize_];

    for(short i = 0; i < rsize_; i++)
      records_[i] = new BTPlayerRecord(*(other.records_[i]));
  }

  return *this;
}

int BTPlayer::operator==(const BTPlayer& other)
{
  return strcmp(key_, other.key_) == 0;
}

int BTPlayer::operator!=(const BTPlayer& other)
{
  return strcmp(key_, other.key_) != 0;
}

int BTPlayer::compareName(const void *left, const void *right)
{
  assert(left != 0);
  assert(right != 0);

  BTPlayer *lval = *((BTPlayer **) left);
  BTPlayer *rval = *((BTPlayer **) right);

  assert(lval != 0);
  assert(rval != 0);

  return strcmp(lval->key_, rval->key_);
}

int BTPlayer::compareRank(const void *left, const void *right)
{
  assert(left != 0);
  assert(right != 0);
 
  BTPlayer *lval = *((BTPlayer **) left);
  BTPlayer *rval = *((BTPlayer **) right);

  assert(lval != 0);
  assert(rval != 0);

  if(lval->rank_ < rval->rank_)
    return 1;

  if(lval->rank_ > rval->rank_)
    return -1;

  return strcmp(lval->key_, rval->key_);
}

#ifndef NDEBUG
ostream& operator<<(ostream& os, BTPlayer& player)
{
  os << "key = <" << player.key_ << ">\n";
  os << "valid = " << player.valid_ << "\n";
  os << "size = " << player.size_ << "\n";
  os << "-----------------------------\n";

  os << "gecos = <" << player.gecos_ << ">\n";
  os << "rank = " << player.rank_ << "\n";
  os << "wins = " << player.wins_ << "\n";
  os << "losses = " << player.losses_ << "\n";
  os << "highScore = " << player.highScore_ << "\n";
  os << "highLines = " << player.highLines_ << "\n";
  os << "highFunds = " << player.highFunds_ <<  "\n";

  os << "streak = " << player.streak_ << "\n";
  switch(player.streakType_) {
  case BTSTREAK_WINS:
    os << "streakType = BTSTREAK_WINS\n";
    break;
  case BTSTREAK_LOSSES:
    os << "streakType = BTSTREAK_LOSSES\n";
    break;
  case BTSTREAK_EMPTY:
    os << "streakType = BTSTREAK_EMPTY\n";
    break;
  default:
    os << "streakType = " << player.streakType_ << "\n";
  }

  os << "fastestKill = " << player.fastestKill_ << "\n";
  os << "quickestDeath = " << player.quickestDeath_ << "\n";
  os << "longestGame = " << player.longestGame_ << "\n";

  os << "rsize = " << player.rsize_ << "\n";

  for(short i = 0; i < player.rsize_; i++)
    os << *(player.records_[i]);
  
  os << endl;
  return os;
}
#endif

void BTPlayer::concatTime(char *buf, long secs)
{
  static char timebuf[10]; // Big enough for hh:mm:ss\0
  long mins, hours;

  mins = secs / 60;
  secs %= 60;
  hours = mins / 60;
  mins %= 60;

  snprintf(timebuf, sizeof(timebuf), "%2.2ld:%2.2ld:%2.2ld",
           hours > 99 ? 99 : hours, mins, secs); 
  strcat(buf, timebuf);
}

char *BTPlayer::formatInfo()
{
  static char buf[2048]; // Big enough for lots of text

  snprintf(buf, sizeof(buf), "          Name: %s\n          Rank: %lu\n          Wins: %lu\n        Losses: %lu\n Highest score: %lu\n Highest lines: %lu\n Highest funds: %lu\n        Streak: %lu",
           gecos_, rank_, wins_, losses_, highScore_, highLines_, highFunds_, streak_);

  if(streakType_ == BTSTREAK_LOSSES) {
    strcat(buf, " loss");
    if(streak_ != 1)
      strcat(buf, "es");
  } else {
    strcat(buf, " win");
    if(streak_ != 1)
      strcat(buf, "s");
  }

  if(fastestKill_ == 0) {
    strcat(buf, "\n  Fastest kill: None");
  } else {
    strcat(buf, "\n  Fastest kill: ");
    concatTime(buf, fastestKill_);
  }

  if(quickestDeath_ == 0) {
    strcat(buf, "\nQuickest death: None");
  } else {
    strcat(buf, "\nQuickest death: ");
    concatTime(buf, quickestDeath_);
  }

  if(longestGame_ == 0) {
    strcat(buf, "\n  Longest game: None");
  } else {
    strcat(buf, "\n  Longest game: ");
    concatTime(buf, longestGame_);
  }

  static char nickname[BT_NICKNAMELEN];
  static char btplan[BT_PLANFILELEN];

  if(plan(nickname, sizeof(nickname), btplan, sizeof(btplan))) {
    strcat(buf, "\n\nNickname: ");
    strcat(buf, nickname);
    strcat(buf, "\nPlan:\n");
    strcat(buf, btplan);
  } else {
    strcat(buf, "\n\nNickname: none\nPlan: none");
  }

  return buf;
}

BTPlayerRecord *BTPlayer::recordAgainst(BTPlayer *opponent)
{
  assert(opponent != 0);

  static BTPlayerRecord key;

  BTPlayerRecord *keyptr = &key;
  BTPlayerRecord **found = 0;

  strncpy(key.opponent_, opponent->key_, BTDBRECORD_KEYLEN);

  if(rsize_ > 0) {
    found = (BTPlayerRecord **) bsearch((const void *) &keyptr,
       (const void *) records_, rsize_, sizeof(BTPlayerRecord *),
       BTPlayerRecord::compare);
  }

  return found ? *found : (BTPlayerRecord *) 0;
}

int BTPlayer::plan(char *nickname, int namelen, char *plan, int planlen)
{
  char namebuf[BTDBRECORD_KEYLEN];
  struct passwd *pwdentry;
  char *ptr;

  strcpy(namebuf, key_);

  if((ptr = strchr(namebuf, '@')))
    *ptr = '\0';

  if((pwdentry = getpwnam(namebuf)) == 0)
    return 0;

  // Make sure to give certain folks a special greeting ...

  if(strcmp(namebuf, "kr") == 0) {
    strcpy(nickname, "Doormat");
    if(plan) strcpy(plan, "I'm in the about box...\nbut I still suck.");
    return 1;
  }

  if(strcmp(namebuf, "jsl") == 0) {
    strcpy(nickname, "The Whining Gnat");
    if(plan) strcpy(plan, "To sue BattleTris for harassment.");
    return 1;
  }

  if(strcmp(namebuf, "jak") == 0) {
    strcpy(nickname, "Paranoid");
    if(plan) strcpy(plan, "I found this secret message by\nrunning strings on the\nBattleTris executable.");
  }

  static char planfile[1024];
  struct stat statbuf;

  bzero((char *) planfile, sizeof(planfile));
  strcpy(planfile, pwdentry->pw_dir);
  strcat(planfile, BTDB_PLANFILE);

  if(stat(planfile, &statbuf) < 0)
    return 0;

  ifstream btplan(planfile);
  
  // Get the user\'s nickname
  btplan.get(nickname, namelen, '\n');

  // If no nickname is specified, then just stick in the login name

  if(strlen(nickname) == 0)
    strncpy(nickname, namebuf, namelen);

  // Get rid of nasty control-character hacks
  validateBuffer(nickname, strlen(nickname));

  if(plan != 0) {
    // Eat characters until we munch a return
    for(char ch = '\0'; (ch != '\n') && (btplan.good()); btplan.get(ch));

    // Everything else up to EOF is the plan message
    btplan.get(plan, planlen, EOF);

    // Get rid of nasty control-character hacks
    validateBuffer(plan, strlen(plan));
  }

  btplan.close();
  return 1;
}

void BTPlayer::validateBuffer(char *buf, int buflen)
{
  for(register int i = 0; i < buflen; i++)
    if((!isprint(buf[i])) && (buf[i] != '\n'))
      buf[i] = '!';
}

short BTPlayer::read(int fd, off_t offset, off_t nbytes)
{
  if(lseek(fd, offset, SEEK_SET) < 0)
    return ERRBTDBSEEK;

  char *buf = new char [nbytes];

  if(::read(fd, (void *) buf, nbytes) != nbytes) {
    delete [] buf;
    return ERRBTDBREAD;
  }

  (void) BTPlayer::readbuf(buf);
  delete [] buf;
  return ERRBTDBNOERR;
}

short BTPlayer::write(int fd, off_t offset)
{
  if(lseek(fd, offset, SEEK_SET) < 0)
    return ERRBTDBSEEK;

  char *buf = new char [size_];

  (void) BTPlayer::writebuf(buf);

  if(writen(fd, (void *) buf, size_) != size_) {
    delete [] buf;
    return ERRBTDBWRITE;
  }

  delete [] buf;
  return ERRBTDBNOERR;
}

char *BTPlayer::writebuf(char *bufptr)
{
  unsigned short ts;
  unsigned long tl;

  bcopy((char *) key_, (char *) bufptr, sizeof(key_));
  bufptr += sizeof(key_);
  bcopy((char *) gecos_, (char *) bufptr, sizeof(gecos_));
  bufptr += sizeof(gecos_);

  BTNET_PUTLONG(bufptr, tl, rank_);
  BTNET_PUTLONG(bufptr, tl, wins_);
  BTNET_PUTLONG(bufptr, tl, losses_);
  BTNET_PUTLONG(bufptr, tl, highScore_);
  BTNET_PUTLONG(bufptr, tl, highLines_);
  BTNET_PUTLONG(bufptr, tl, highFunds_);
  BTNET_PUTLONG(bufptr, tl, streak_);
  BTNET_PUTSHORT(bufptr, ts, streakType_);
  BTNET_PUTLONG(bufptr, tl, fastestKill_);
  BTNET_PUTLONG(bufptr, tl, quickestDeath_);
  BTNET_PUTLONG(bufptr, tl, longestGame_);
  BTNET_PUTSHORT(bufptr, ts, rsize_);

  BTPlayerRecord *record = 0;

  for(int i = 0; i < rsize_; i++) {
    record = records_[i];

    bcopy((char *) record->opponent_, (char *) bufptr,
          sizeof(record->opponent_));

    bufptr += sizeof(record->opponent_);
    BTNET_PUTLONG(bufptr, tl, record->wins_);
    BTNET_PUTLONG(bufptr, tl, record->losses_);
  }

  return bufptr;
}

char *BTPlayer::readbuf(char *bufptr)
{
  unsigned short ts;
  unsigned long tl;

  if(records_) {
    for(short i = 0; i < rsize_; i++)
      delete records_[i];
    delete [] records_;
    records_ = 0;
  }

  char *bufstart = bufptr;

  bcopy((char *) bufptr, (char *) key_, sizeof(key_));
  bufptr += sizeof(key_);
  bcopy((char *) bufptr, (char *) gecos_, sizeof(gecos_));
  bufptr += sizeof(gecos_);

  BTNET_GETLONG(bufptr, tl, rank_);
  BTNET_GETLONG(bufptr, tl, wins_);
  BTNET_GETLONG(bufptr, tl, losses_);
  BTNET_GETLONG(bufptr, tl, highScore_);
  BTNET_GETLONG(bufptr, tl, highLines_);
  BTNET_GETLONG(bufptr, tl, highFunds_);
  BTNET_GETLONG(bufptr, tl, streak_);
  BTNET_GETSHORT(bufptr, ts, streakType_);
  BTNET_GETLONG(bufptr, tl, fastestKill_);
  BTNET_GETLONG(bufptr, tl, quickestDeath_);
  BTNET_GETLONG(bufptr, tl, longestGame_);
  BTNET_GETSHORT(bufptr, ts, rsize_);

  if(rsize_ > 0) {
    records_ = new BTPlayerRecord* [rsize_];
    BTPlayerRecord *record = 0;

    for(int i = 0; i < rsize_; i++) {
      record = new BTPlayerRecord;
      records_[i] = record;

      bcopy((char *) bufptr, (char *) record->opponent_,
            sizeof(record->opponent_));

      bufptr += sizeof(record->opponent_);
      BTNET_GETLONG(bufptr, tl, record->wins_);
      BTNET_GETLONG(bufptr, tl, record->losses_);
    }

    qsort((void *) records_, rsize_, sizeof(BTPlayerRecord *),
	  BTPlayerRecord::compare);
  }

  size_ = bufptr - bufstart;
  valid_ = 1;

  return bufptr;
}

void BTPlayer::recordWin(BTGameStats *stats, BTPlayer *opponent)
{
  assert(stats != 0);
  assert(opponent != 0);

  // Update the winner\'s rank using the ELO chess ranking system

  rank_ = recompute_elo_rank(rank_, opponent->rank_, 1);

  // Update all the statistics we keep track of using the BTGameStats object

  if(stats->winnerScore_ > highScore_)
    highScore_ = stats->winnerScore_;

  if(stats->winnerLines_ > highLines_)
    highLines_ = stats->winnerLines_;

  if(stats->winnerFunds_ > highFunds_) {
    if(strcmp(key(), "bmc") == 0) {
      if(stats->winnerFunds_ < 1454) // Don't let Bry go higher than 1453
	highFunds_ = stats->winnerFunds_;
    } else if(strcmp(key(), "cgh") == 0) {
      if(stats->winnerFunds_ < 1425) // Don't let Chuck go higher than 1424
        highFunds_ = stats->winnerFunds_;
    } else if(strcmp(key(), "mws") == 0) {
      if(stats->winnerFunds_ < 1403) // CJ "db-boy" no higher than 1402
        highFunds_ = stats->winnerFunds_;
    } else {
      highFunds_ = stats->winnerFunds_;
    }
  }

  if(stats->duration_ > longestGame_)
    longestGame_ = stats->duration_;

  if((fastestKill_ == 0) || (stats->duration_ < fastestKill_))
    fastestKill_ = stats->duration_;

  // Update my number of wins and my current winning streak

  wins_++;

  if(streakType_ == BTSTREAK_WINS)
    streak_++;
  else {
    streakType_ = BTSTREAK_WINS;
    streak_ = 1;
  }

  // Update my head-to-head record against this opponent.  First we call
  // bsearch to search the sorted array of BTPlayerRecord object pointers,
  // to see if we have played this opponent before.  If we have, we just
  // modify this BTPlayerRecord to indicate another win.  If we\'ve never
  // played this opponent before, we need to grow the array and our record
  // size accordingly, and add a new BTPlayerRecord object for this opponent

  static BTPlayerRecord key;
  BTPlayerRecord *keyptr = &key;
  BTPlayerRecord **opponentHdl = 0;

  strncpy(key.opponent_, opponent->key_, BTDBRECORD_KEYLEN);

  if(rsize_ > 0) {
    opponentHdl = (BTPlayerRecord **) bsearch((const void *) &keyptr,
       (const void *) records_, rsize_, sizeof(BTPlayerRecord *),
       BTPlayerRecord::compare);
  }

  if(opponentHdl == 0) {	// Case 1: We\'ve never played this opponent

    // Allocate a new records array which is one slot bigger and copy in the
    // contents of the old records array

    BTPlayerRecord **newRecords = new BTPlayerRecord* [rsize_ + 1];
    
    // Blow away our records array and replace it with the new array, and
    // be sure to update rsize_ to match the new size of our records array

    if(records_ != 0) {
      bcopy((char *) records_, (char *) newRecords,
            sizeof(BTPlayerRecord *) * rsize_);
      delete [] records_;
    }

    records_ = newRecords;
    rsize_++;

    // Allocate a new BTPlayerRecord object and put it at the end of the
    // array, and increment our record size accordingly

    records_[rsize_ - 1] = new BTPlayerRecord(opponent->key_);

    records_[rsize_ - 1]->wins_ = 1;
    size_ += records_[rsize_ - 1]->datalen();

    // Since we used bsearch to determine whether or not we\'ve played this
    // opponent before, we have to be sure to resort the records array
    // whenever we modify it

    qsort((void *) records_, rsize_, sizeof(BTPlayerRecord *),
	  BTPlayerRecord::compare);
	 
  } else {			// Case 2: We\'ve played this opponent before

    // If we\'ve already played this opponent before, just credit another win
    // This guy is probably a doormat anyway ...

    (*opponentHdl)->wins_++;
  }
}

void BTPlayer::recordLoss(BTGameStats *stats, BTPlayer *opponent)
{
  assert(stats != 0);
  assert(opponent != 0);

  // Update the winner\'s rank using the ELO chess ranking system

  rank_ = recompute_elo_rank(rank_, opponent->rank_, 0);

  // Update all the statistics we track using the BTGameStats object contents

  if(stats->loserScore_ > highScore_)
    highScore_ = stats->loserScore_;

  if(stats->loserLines_ > highLines_)
    highLines_ = stats->loserLines_;

  if(stats->loserFunds_ > highFunds_) {
    if(strcmp(key(), "bmc") == 0) {
      if(stats->loserFunds_ < 1454) // Don't let Bry go higher than 1453
	highFunds_ = stats->loserFunds_;
    } else if(strcmp(key(), "cgh") == 0) {
      if(stats->loserFunds_ < 1425) // Don't let Chuck go higher than 1424
        highFunds_ = stats->loserFunds_;
    } else if(strcmp(key(), "mws") == 0) {
      if(stats->loserFunds_ < 1403) // CJ "db-boy" no higher than 1402
        highFunds_ = stats->loserFunds_;
    } else {
      highFunds_ = stats->loserFunds_;
    }
  }

  if(stats->duration_ > longestGame_)
    longestGame_ = stats->duration_;

  if((quickestDeath_ == 0) || (stats->duration_ < quickestDeath_))
    quickestDeath_ = stats->duration_;

  // Update my losses and my current losing streak

  losses_++;

  if(streakType_ == BTSTREAK_LOSSES)
    streak_++;
  else {
    streakType_ = BTSTREAK_LOSSES;
    streak_ = 1;
  }

  // Update my head-to-head record against this opponent.  First we call
  // bsearch to search the sorted array of BTPlayerRecord object pointers,
  // to see if we have played this opponent before.  If we have, we just
  // modify this BTPlayerRecord to indicate another loss.  If we\'ve never
  // played this opponent before, we need to grow the array and our record
  // size accordingly, and add a new BTPlayerRecord object for this opponent

  static BTPlayerRecord key;
  BTPlayerRecord *keyptr = &key;
  BTPlayerRecord **opponentHdl = 0;

  strncpy(key.opponent_, opponent->key_, BTDBRECORD_KEYLEN);

  if(rsize_ > 0) {
    opponentHdl = (BTPlayerRecord **) bsearch((const void *) &keyptr,
       (const void *) records_, rsize_, sizeof(BTPlayerRecord *), 
       BTPlayerRecord::compare);
  }

  if(opponentHdl == 0) {	// Case 1: We\'ve never played this opponent

    // Allocate a new records array which is one slot bigger and copy in the
    // contents of the old records array

    BTPlayerRecord **newRecords = new BTPlayerRecord* [rsize_ + 1];
    
    // Blow away our records array and replace it with the new array, and
    // be sure to update rsize_ to match the new size of our records array

    if(records_) {
      bcopy((char *) records_, (char *) newRecords,
	     sizeof(BTPlayerRecord *) * rsize_);
      delete [] records_;
    }

    records_ = newRecords;
    rsize_++;

    // Allocate a new BTPlayerRecord object and put it at the end of the
    // array, and increment our record size accordingly

    records_[rsize_ - 1] = new BTPlayerRecord(opponent->key_);

    records_[rsize_ - 1]->losses_ = 1;
    size_ += records_[rsize_ - 1]->datalen();

    // Since we used bsearch to determine whether or not we\'ve played this
    // opponent before, we have to be sure to resort the records array
    // whenever we modify it

    qsort((void *) records_, rsize_, sizeof(BTPlayerRecord *),
          BTPlayerRecord::compare);
	 
  } else {			// Case 2: We\'ve played this opponent before

    // If we\'ve already played this opponent before, just credit another loss
    // You\'ve won this time Gadget, but I\'ll be baackkkk ...

    (*opponentHdl)->losses_++;
  }
}
