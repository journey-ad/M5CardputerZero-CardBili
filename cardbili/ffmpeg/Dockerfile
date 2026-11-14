# docker build --platform linux/arm64 --output type=local,dest=dist/ffmpeg cardbili/ffmpeg
FROM registry-1.docker.io/library/alpine:3.23 AS build
RUN apk add --no-cache build-base linux-headers openssl-dev openssl-libs-static pkgconf xz
ADD --checksum=sha256:de668509caf9e35e3cd162473441fdb29538c6d96ed080292b3cf9e6fc5d558f \
    https://ffmpeg.org/releases/ffmpeg-7.1.5.tar.xz /tmp/ffmpeg.tar.xz
WORKDIR /build
RUN tar -xf /tmp/ffmpeg.tar.xz --strip-components=1
COPY args.c fftools/cardbili_args.c
COPY CHANGES.md CARDBILI-CHANGES.md
RUN sed -i 's/^int main(int argc, char \*\*argv)/int ffmpeg_main(int argc, char **argv);\nint ffmpeg_main(int argc, char **argv)/' fftools/ffmpeg.c \
    && sed -i '/^OBJS-ffmpeg +=/i OBJS-ffmpeg += fftools/cardbili_args.o' fftools/Makefile \
    && tar -cJf /tmp/ffmpeg-source.tar.xz -C /build .
RUN ./configure \
    --disable-autodetect --disable-everything \
    --disable-shared --enable-static --enable-small --disable-debug --disable-doc \
    --disable-ffplay --disable-ffprobe --disable-avdevice --disable-postproc \
    --enable-ffmpeg --enable-network --enable-openssl --enable-version3 \
    --enable-protocol=file,pipe,http,https,tcp,tls,crypto \
    --enable-demuxer=mov --enable-parser=h264,aac \
    --enable-decoder=h264,aac --enable-encoder=rawvideo,pcm_s16le \
    --enable-muxer=rawvideo,pcm_s16le \
    --enable-filter=setpts,atempo,fps,scale,pad,format,aformat,aresample \
    --extra-ldflags=-static --pkg-config-flags=--static --extra-cflags=-O3 \
    && make -j4 ffmpeg \
    && mkdir -p /out/licenses/ffmpeg \
    && install -m755 ffmpeg /out/cardbili-ffmpeg \
    && strip /out/cardbili-ffmpeg \
    && cp COPYING.LGPLv3 COPYING.GPLv3 LICENSE.md /out/licenses/ffmpeg/ \
    && cp /tmp/ffmpeg-source.tar.xz /out/ffmpeg-source.tar.xz \
    && ./ffmpeg -version > /out/build-info.txt \
    && apk info -v musl openssl libgcc >> /out/build-info.txt
ADD --checksum=sha256:7d5450cb2d142651b8afa315b5f238efc805dad827d91ba367d8516bc9d49e7a \
    https://raw.githubusercontent.com/openssl/openssl/openssl-3.5.9/LICENSE.txt /out/licenses/openssl/LICENSE.txt
FROM scratch
COPY --from=build /out/ /
COPY licenses/ /licenses/runtime/
