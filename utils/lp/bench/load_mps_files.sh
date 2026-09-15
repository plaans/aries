git clone --depth 1 https://github.com/ozy4dm/lp-data-netlib.git

mv lp-data-netlib/mps_files .

rm -rf lp-data-netlib

# We remove the mps files that are not handled by our parser
rm -f mps_files/forplan.mps mps_files/pilot4.mps
