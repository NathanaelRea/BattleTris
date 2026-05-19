#include "BTConfig.H"

#if STDC_HEADERS
# include <stdlib.h>
# include <ctype.h>
#else
# define isspace(x) ((x == ' ') || (x == '\t'))
#endif


#include <iostream>
using namespace std;
#include <stdio.h>
#include <math.h>

#include <Xm/Xm.h>

#include "PPMReader.H"

PPMReader::PPMReader(Widget widget, const char *root)
: widget_(widget), range_(5), num_colors_(125), 
  color_cells_(new XColor [125]), valid_(1)
{
  unsigned char red, green, blue;
  int counter = 0;

  if(root) 
    strcpy(root_, root);
  else
    root_[0] = 0;

  XtVaGetValues(widget, XtNcolormap, &cmap_, 
    XtNvisual, &visual_, 
    XtNdepth, &depth_,
    0);

  Display *display = XtDisplay(widget_);

  for(red = 0; red < range_; red++) {
    for(green = 0; green < range_; green++) {
      for(blue = 0; blue < range_; blue ++) {
        color_cells_[counter].flags = DoRed | DoGreen | DoBlue;
        color_cells_[counter].red = ((red*PPM_USHRT_MAX)/(range_-1));
        color_cells_[counter].green = ((green*PPM_USHRT_MAX)/(range_-1));
        color_cells_[counter].blue = ((blue*PPM_USHRT_MAX)/(range_-1));

      if(!XAllocColor(display, cmap_, &color_cells_[counter])) {
        valid_ = 0;
        return;
      }
	    counter++;

      }
    }
  }	

  for(int i = 0; i < num_colors_; i++) 
    pixels_[i] = color_cells_[i].pixel;
}

int PPMReader::getNextNum(FILE *fp)
{
  char current;
  char buf[25];
  int counter = 0;
  int num;
  char* ptr[1];

  fread(&current, sizeof(char), 1, fp);

  while (current == '#') { 
    do {
      fread(&current, sizeof(char), 1, fp);
    } while(current != '\n');
    fread(&current, sizeof(char), 1, fp);
  }

  while(isspace(current)) {
    fread(&current, sizeof(char), 1, fp);
  }
    
  while(!isspace(current)) {
    buf[counter] = current;
    counter++;
    fread(&current, sizeof(char), 1, fp);
  }

  buf[counter] = '\000';
  num = (int) strtod (buf, ptr);

  if(**ptr == '\000')
    return num;
  else
    return PPM_INVALID_NUM;
}
    
int PPMReader::readPpmFile(const char *filename, PPMPixel *& data, 
			   int& width, int& height)
{
  char fname[1024];
  FILE *fp;

  char buf[2048], magic[3];
  int maxval;

  if(!valid_) {
    cerr << "BattleTris: Failed to initialize PPM reader" << endl;
    return 0;
  }

  strcpy(fname, root_);
  strcat(fname, filename);

  if(!(fp = fopen(fname, "r"))) {
    cerr << "BattleTris: Failed to open PPM file " << fname << endl;
    return 0;
  }

  fread(magic, sizeof(char), 2, fp);
  magic[2] = '\000';

  if(strcmp(magic, "P6") != 0 && strcmp (magic, "P3")) {
    cerr << "BattleTris: Invalid PPM header for file " << fname << endl;
    return 0;
  }
    
  fread(buf, sizeof(char), 1, fp);
  fgets(buf, 2048, fp);

  width = getNextNum(fp);
  height = getNextNum(fp);
  maxval = getNextNum(fp);

  if(width == PPM_INVALID_NUM) {
    cerr << "BattleTris: Invalid PPM width for file " << fname << endl;
    return 0;
  }

  if(height == PPM_INVALID_NUM) {
    cerr << "BattleTris: Invalid PPM height for file " << fname << endl;
    return 0;
  }

  if(maxval == PPM_INVALID_NUM) {
    cerr << "BattleTris: Invalid PPM maxval for file " << fname << endl;
    return 0;
  }

  data = new PPMPixel[width * height];

  int imagelen = (width * height * 3 * sizeof(char));
  char *image = new char [imagelen];

  memset((void *) image, 0, imagelen);

  if(fread(image, imagelen - 1, 1, fp) == 0)
    cerr << "BattleTris: Failed to read PPM data for " << fname << endl;
  
  if(maxval == PPM_CHAR_MAX) {

    // Since all of our BattleTris PPM files use 255 as maxval, we can
    // just do this in one memcpy --mws

    memcpy((void *) data, (void *) image, imagelen);

  } else {
    for(int counter = 0; counter < (width * height); counter++) {

      data[counter].blue_ = (unsigned char)
	(((float) image[counter * 3 + 1] /
	  (float) maxval ) * PPM_CHAR_MAX);

      data[counter].red_ = (unsigned char)
	(((float) image[counter * 3 + 2] /
	  (float) maxval) * PPM_CHAR_MAX);
      
      data[counter].green_ = (unsigned char)
	(((float) image[counter*3] /
	  (float) maxval) * PPM_CHAR_MAX);
    }
  }
	
  delete [] image;
  fclose(fp);

  return 1;
}
    
XImage *PPMReader::createImage(PPMPixel *data, const int width,
			       const int height)
{
  if(!valid_) {
    cerr << "BattleTris: Failed to initialize PPM reader" << endl;
    return NULL;
  }

  XImage *image = XCreateImage 
    (XtDisplay (widget_), visual_, depth_, ZPixmap, 0, 0, width, height,
    XBitmapPad (XtDisplay (widget_)), 0);

  char *image_data = image->data = new char 
    [image->bytes_per_line * image->height];
  
  double increment = (PPM_CHAR_MAX / (range_ - 1));
  register int i;

  for(i = 0; i < num_colors_; i++) 
    need_[i] = 0;

  long color;

  for(i = 0; i < (width * height); i++) {
    color = ((int) (data[i].red_ / increment)) * range_ * range_ + 
      ((int) (data[i].green_ / increment)) * range_ +
      ((int) (data[i].blue_ / increment));
    need_[color] = 1;
    XPutPixel (image, i % width, i / width, pixels_[color]);
  }

  delete [] data;
    
  return image;
}

PPMReader::~PPMReader()
{
  if(valid_)
    delete [] color_cells_;
}
