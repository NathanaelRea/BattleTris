/****************************************************************/
/*    NAME: Bryan Cantrill                                      */
/*    ACCT: bmc                                                 */
/*    FILE: BTScore.H                                           */
/*    ASSN:                                                     */
/*    DATE: Sun Feb 20 21:09:23 1994                            */
/****************************************************************/

#ifndef BTSCORE_HEADER
#define BTSCORE_HEADER

#include "BTConfig.H"


#include <iostream>
using namespace std;

class BTScore {
friend ostream& operator<< (ostream& s, BTScore &score);
 
public:
  unsigned long score_, op_score_; 
  unsigned long lines_, op_lines_;
  long funds_, op_funds_;

public:
  BTScore()
  : score_ (0), op_score_ (0), lines_ (0), op_lines_ (0),
    funds_(0), op_funds_ (0) {}
  ~BTScore() {}

  char *writebuf(char *bufptr);
  char *readbuf(char *bufptr);

  void clear() { 
    score_ = 0; op_score_ = 0; lines_ = 0; op_lines_ = 0; 
    funds_ = 0; op_funds_ = 0;
  }
};  

inline ostream& operator<< (ostream& s, BTScore& score)
{
  s << "Current score     : " << score.score_ << endl;
  s << "Opponent's score  : " << score.op_score_ << endl;
  s << "Current lines     : " << score.lines_ << endl;
  s << "Opponent's lines  : " << score.op_lines_ << endl;
  s << "Funds             : " << score.funds_ << endl << endl;
  return s;
}
    
#endif
