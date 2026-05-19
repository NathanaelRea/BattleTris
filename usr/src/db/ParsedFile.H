#ifndef PARSEDFILE_H
#define PARSEDFILE_H

#include "BTConfig.H"


#include <fstream>
using namespace std;

class ParsedFile {

  enum {
    PF_LINEBUF = 2048
  };

protected:
  char *tokens_[PF_LINEBUF];
  char buf_[PF_LINEBUF];
  int ntokens_;
  int tokidx_;
  int eof_;
  ifstream is_;

public:
  ParsedFile(const char *pathname)
  : ntokens_(0), tokidx_(0), is_(pathname), eof_(0) {}

  ~ParsedFile() {}

  int parseline(char comment = '#');

  char *token() {
    if(tokidx_ < ntokens_)
      return tokens_[tokidx_++];
    return (char *) 0;
  }

  int ntokens() { return ntokens_; }

  int fail() { return is_.fail(); }
  int eof() { return is_.eof() || eof_; }
};

#endif
