#
#     NAME: Makefile
#   AUTHOR: Michael Shapiro
#     DATE: 14Oct94
#    DESCR: Makefile for BattleTris source tree
# MODIFIED: 4Nov95
#

include ./Makeinclude

all: config src

config: dirs ../include/BTConfig.H

dirs:
	@mkdir -p ../include ../lib ../bin

../include/BTConfig.H: BTConfig.H
	@echo "+++ installing BattleTris configuration file"
	sh ../bin/install-sh -c -m 0444 BTConfig.H ../include/BTConfig.H

src: dirs
	@echo "+++ building BattleTris include directory"
	@( cd game && $(MAKE) common )
	@echo "+++ building BattleTris standard library"
	@( cd stdlib && $(MAKE) install )
	@echo "+++ building BattleTris signals library"
	@( cd signals && $(MAKE) install )
	@echo "+++ building BattleTris audio library"
	@( cd audio && $(MAKE) install )
	@echo "+++ building BattleTris sockets library"
	@( cd sockets && $(MAKE) install )
	@echo "+++ building BattleTris database library"
	@( cd db && $(MAKE) install )
	@echo "+++ building BattleTris widget library"
	@( cd widget && $(MAKE) install )
	@echo "+++ building BattleTris daemons"
	@( cd daemons && $(MAKE) )
	@echo "+++ building BattleTris referee"
	@( cd btref && $(MAKE) )
	@echo "+++ building BattleTris client"
	@( cd game && $(MAKE) )

install:
	#@echo "+++ installing BattleTris documentation"
	#@( cd man && $(MAKE) install )
	@echo "+++ installing BattleTris shared files"
	@( cd share && $(MAKE) install )
	@echo "+++ installing BattleTris daemons"
	@( cd daemons && $(MAKE) install )
	@echo "+++ installing BattleTris referee"
	@( cd btref && $(MAKE) install )
	@echo "+++ installing BattleTris client"
	@( cd game && $(MAKE) install )

clean:
	@( cd game && $(MAKE) clean )
	@( cd stdlib && $(MAKE) clean )
	@( cd audio && $(MAKE) clean )
	@( cd db && $(MAKE) clean )
	@( cd signals && $(MAKE) clean )
	@( cd sockets && $(MAKE) clean )
	@( cd widget && $(MAKE) clean )
	@( cd daemons && $(MAKE) clean )
	@( cd btref && $(MAKE) clean )
	@( cd game && $(MAKE) clean )

distclean:
	$(RM) config.status config.log config.cache Makeinclude BTConfig.H

CSDIR   = .
CSDIRS  = btref daemons db game share signals sockets stdlib widget
CSPATHS = $(CSDIRS:%=$(CSDIR)/%)
CSINCS  = $(CSPATHS:%=-I%)
CSCOPE  = cscope

#
# Set CSFLAGS env variable to -bq when using fast cscope to
# build the fast (but large) cscope data bases.
#
CSFLAGS = -b

.PRECIOUS:      cscope.out

cscope.out: cscope.files
	${CSCOPE} ${CSFLAGS}

cscope.files:
	@-$(RM) cscope.files cscope.files.raw
	echo "$(CSINCS)" > cscope.files
	-find $(CSPATHS) -name SCCS -prune -o \
	    -type d -name '.del-*' -prune -o -type f \
	    \( -name '*.[Ccshlxy]' -o -name 'Makefile*' -o -name '*.il*' \
	    -o -name '*.cc' -o -name '*.adb' \) \
	    -print > cscope.files.raw
	grep -v Makefile cscope.files.raw >> cscope.files
	grep Makefile cscope.files.raw >> cscope.files
	-$(RM) cscope.files.raw
	@wc -l cscope.files
