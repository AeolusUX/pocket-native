#include "overrides.h"

/* the title's LZ10/LZ11 decompressor, (source, source size, destination),
   0 when it worked, -1 for another kind of data, -2 when the source ran
   out, -3 when more than 32 bytes of it were left, -4 for a copy from
   before the destination or past the size */
RECOMP_OVERRIDE(0x0015F800) {
    uint32_t src = ctx->r[0], left = ctx->r[1], dst = ctx->r[2], start = ctx->r[2];
    uint32_t type = mem_read8(ctx, src);
    uint32_t size = mem_read32(ctx, src) >> 8;
    int wide = (type & 0xF) != 0;
    int32_t result = 0;
    if ((type & 0xF0) != 0x10 || (type & 0xF) > 1) {
        ctx->r[0] = (uint32_t)-1;
        RETURN_TO(ctx->r[14]);
    }
    if (left <= 4) {
        ctx->r[0] = (uint32_t)-2;
        RETURN_TO(ctx->r[14]);
    }
    src += 4;
    int32_t remaining = (int32_t)(left - 4);
    if (size == 0) {
        if (remaining < 4) {
            ctx->r[0] = (uint32_t)-2;
            RETURN_TO(ctx->r[14]);
        }
        size = mem_read32(ctx, src);
        src += 4;
        remaining -= 4;
    }
    while (size != 0) {
        uint32_t flags = mem_read8(ctx, src++);
        if (--remaining < 0) {
            result = -2;
            goto done;
        }
        for (int bit = 0; bit < 8 && size != 0; bit++, flags <<= 1) {
            if (!(flags & 0x80)) {
                uint8_t value = mem_read8(ctx, src++);
                remaining--;
                if (remaining >= 0)
                    size--;
                mem_write8(ctx, dst++, value);
                if (remaining < 0) {
                    result = -2;
                    goto done;
                }
                continue;
            }
            uint32_t first = mem_read8(ctx, src);
            uint32_t length = first >> 4;
            if (!wide) {
                length += 3;
            } else if (length == 1) {
                uint32_t b1 = mem_read8(ctx, src + 1), b2 = mem_read8(ctx, src + 2);
                length = (((first & 0xF) << 12) | (b1 << 4) | (b2 >> 4)) + 0x111;
                src += 2;
                remaining -= 2;
            } else if (length == 0) {
                uint32_t b1 = mem_read8(ctx, src + 1);
                length = (((first & 0xF) << 4) | (b1 >> 4)) + 0x11;
                src += 1;
                remaining -= 1;
            } else {
                length += 1;
            }
            uint32_t high = mem_read8(ctx, src++);
            uint32_t low = mem_read8(ctx, src++);
            remaining -= 2;
            uint32_t distance = (((high << 28) >> 20) | low) + 1;
            if (remaining < 0) {
                result = -2;
                goto done;
            }
            if (length > size || dst - distance < start) {
                result = -4;
                goto done;
            }
            size -= length;
            for (uint32_t i = 0; i < length; i++, dst++)
                mem_write8(ctx, dst, mem_read8(ctx, dst - distance));
        }
    }
    result = remaining > 32 ? -3 : 0;
done:
    ctx->r[3] = dst;
    ctx->r[0] = (uint32_t)result;
    RETURN_TO(ctx->r[14]);
}
