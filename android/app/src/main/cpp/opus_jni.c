// Small bridge so the Kotlin app can use the Opus voice codec.
#include <jni.h>
#include <stdint.h>
#include <opus.h>

JNIEXPORT jlong JNICALL
Java_com_squadtalk_app_Opus_encoderCreate(JNIEnv *env, jclass cls, jint bitrate) {
    int err = 0;
    OpusEncoder *e = opus_encoder_create(48000, 1, OPUS_APPLICATION_VOIP, &err);
    if (err != OPUS_OK || e == NULL) return 0;
    opus_encoder_ctl(e, OPUS_SET_BITRATE(bitrate));
    return (jlong)(intptr_t)e;
}

JNIEXPORT jint JNICALL
Java_com_squadtalk_app_Opus_encode(JNIEnv *env, jclass cls, jlong h, jshortArray pcm, jbyteArray out) {
    if (h == 0) return -1;
    jsize frames = (*env)->GetArrayLength(env, pcm);
    jsize cap = (*env)->GetArrayLength(env, out);
    jshort *p = (*env)->GetShortArrayElements(env, pcm, NULL);
    jbyte *o = (*env)->GetByteArrayElements(env, out, NULL);
    int n = opus_encode((OpusEncoder *)(intptr_t)h, p, frames, (unsigned char *)o, cap);
    (*env)->ReleaseShortArrayElements(env, pcm, p, JNI_ABORT);
    (*env)->ReleaseByteArrayElements(env, out, o, 0);
    return n;
}

JNIEXPORT void JNICALL
Java_com_squadtalk_app_Opus_encoderDestroy(JNIEnv *env, jclass cls, jlong h) {
    if (h != 0) opus_encoder_destroy((OpusEncoder *)(intptr_t)h);
}

JNIEXPORT jlong JNICALL
Java_com_squadtalk_app_Opus_decoderCreate(JNIEnv *env, jclass cls) {
    int err = 0;
    OpusDecoder *d = opus_decoder_create(48000, 1, &err);
    if (err != OPUS_OK || d == NULL) return 0;
    return (jlong)(intptr_t)d;
}

JNIEXPORT jint JNICALL
Java_com_squadtalk_app_Opus_decode(JNIEnv *env, jclass cls, jlong h, jbyteArray data,
                                   jint offset, jint len, jshortArray out) {
    if (h == 0) return -1;
    jsize cap = (*env)->GetArrayLength(env, out);
    jbyte *in = (*env)->GetByteArrayElements(env, data, NULL);
    jshort *o = (*env)->GetShortArrayElements(env, out, NULL);
    int n = opus_decode((OpusDecoder *)(intptr_t)h, (const unsigned char *)(in + offset), len, o, cap, 0);
    (*env)->ReleaseByteArrayElements(env, data, in, JNI_ABORT);
    (*env)->ReleaseShortArrayElements(env, out, o, 0);
    return n;
}

JNIEXPORT void JNICALL
Java_com_squadtalk_app_Opus_decoderDestroy(JNIEnv *env, jclass cls, jlong h) {
    if (h != 0) opus_decoder_destroy((OpusDecoder *)(intptr_t)h);
}
