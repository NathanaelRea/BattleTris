/****************************************************************/
/*    NAME: Bryan Cantrill                                      */
/*    ACCT: bmc                                                 */
/*    FILE: BTBoard.H                                           */
/*    ASSN:                                                     */
/*    DATE: Tue Feb  8 21:21:01 1994                            */
/****************************************************************/

#ifndef BTBOARD_H
#define BTBOARD_H

#include "BTConfig.H"


#include <iostream>
using namespace std;

#include "Block.H"
#include "BTProtocol.H"

class BTBoardManager;

class BTBoard { 
friend class BTCommManager;
friend class BTRecon;
friend class BTBoardManager;
friend ostream& operator<< (ostream &s, BTBoard &board);
private:
  BTWeaponToken motivation_;
  int height_, width_;
  Block < int > rep_;
public:
  BTBoard (BTBoardManager *board, int upsidedown = 0);
  BTWeaponToken motivation() { return motivation_; }
  void motivation (BTWeaponToken motive) { motivation_ = motive; }
  BTBoard (void) : height_(0), width_(0) {}
};
  
#endif
