#!/bin/sh

/tsdb-engine &

exec nginx -g 'daemon off;'
