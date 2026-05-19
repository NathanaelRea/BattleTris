/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: BTSigHandlers.H                                     */
/*    DATE: Tue May  3 01:59:10 1994                            */
/****************************************************************/

#ifndef BTSIGHANDLERS_H
#define BTSIGHANDLERS_H

#include "BTConfig.H"


#include <iostream>
using namespace std;

#if HAVE_UNISTD_H
# include <unistd.h>
#endif

#include "SigHandler.H"
#include "BTStartup.H"
#include "BattleTris.H"

static Boolean termsig_received = False;

class BTSigTermHandler : public SigHandler {
public:
  BTSigTermHandler(SigDisposition disp = SIG_ENABLED)
  : SigHandler(disp) {}
  virtual ~BTSigTermHandler() {}

  void handle() {
    // If we\'ve already been called, don't write anything to the
    // pipe -- we might block on the write since we own both ends of it

    if(termsig_received == False) {
      int packet = 1;
      write(g_signalfds[1], (char *) &packet, sizeof(packet));
      termsig_received = True;
    }
  }
};

class BTSigPipeHandler : public SigHandler {
public:
  BTSigPipeHandler(SigDisposition disp = SIG_ENABLED)
  : SigHandler(disp) {}
  virtual ~BTSigPipeHandler() {}

  void handle() {
    if(g_startup)
      g_startup->childDied();
  }
};

#endif
