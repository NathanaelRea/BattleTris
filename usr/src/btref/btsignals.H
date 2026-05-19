#ifndef BTSIGNALS_H
#define BTSIGNALS_H

#include "BTConfig.H"


#include <iostream>
using namespace std;

#if STDC_HEADERS
# include <stdlib.h>
#endif

#include "SigHandler.H"

class BTRefTermHandler : public SigHandler {

public:
  BTRefTermHandler(SigDisposition disp = SIG_ENABLED)
  : SigHandler(disp) {}

  virtual ~BTRefTermHandler() {}

  void handle() {
    if(maskint)
      cerr << "btref: Interrupt ignored in critical section" << endl;
    else
      exit(0);
  }
};

#endif
