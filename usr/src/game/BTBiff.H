/****************************************************************/
/*    NAME: Bryan Cantrill                                      */
/*    ACCT: bmc                                                 */
/*    FILE: BTBiff.H                                            */
/*    ASSN:                                                     */
/*    DATE: Sat Apr 30 21:12:37 1994                            */
/****************************************************************/

#ifndef BTBIFF_H
#define BTBIFF_H

#include "BTConfig.H"

#include <Xm/MainW.h>
#include <Xm/DialogS.h>

#include <stdio.h>

#include <iostream>
using namespace std;

#include "DevAudio.H"
#include "BTWidget.H"
#include "BTDrawingAreaWidget.H"
#include "BTBiff.H"

class BTPixmap;

class BTBiff {
private:
  DevAudio *dev_;
  BTPixmap *sleep_image_, *chal_image_;
  BTWidget *shell_;
  int sleep_;

public:
  BTBiff(BTWidget *parent, BTPixmap *sleep, BTPixmap *chal, DevAudio *dev);
  Widget getWidget() { return sleep_daw_->getWidget(); }
  void changeBiff();
  int asleep() { return sleep_; }
  ~BTBiff();
  void show() { sleep_daw_->manage(); }
  void hide() { sleep_daw_->unmanage(); }
  BTWIDGET_CALLBACK (BTBiff, handleExpose)
  BTDrawingAreaWidget *sleep_daw_, *chal_daw_;
};

#endif
