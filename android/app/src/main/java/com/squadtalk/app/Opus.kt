package com.squadtalk.app

/** The Opus voice codec (native library built from the same source the PC app uses). */
object Opus {
    init {
        System.loadLibrary("squadtalk_opus")
    }

    @JvmStatic external fun encoderCreate(bitrate: Int): Long
    @JvmStatic external fun encode(handle: Long, pcm: ShortArray, out: ByteArray): Int
    @JvmStatic external fun encoderDestroy(handle: Long)

    @JvmStatic external fun decoderCreate(): Long
    @JvmStatic external fun decode(handle: Long, data: ByteArray, offset: Int, len: Int, out: ShortArray): Int
    @JvmStatic external fun decoderDestroy(handle: Long)
}
