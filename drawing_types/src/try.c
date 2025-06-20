#include "../drawing_types.h"

int main(int argc, char **argv) {
  Entity e;
  e.tag = SOLID_TRIANGLE;
  SolidTriangle_Body b = { .paint = 0, .triangle = {}};
  e.SOLID_TRIANGLE = b;
  debug_print_entity(&e);
  Vec2BufBuilder* builder = vec2_buf_builder_new();
  {
    vec2_buf_builder_handle(builder, (Vec2){ .x = 0., .y = 0.});
    vec2_buf_builder_handle(builder, (Vec2){ .x = 42., .y = 69.});
  }
  {
    vec2_buf_builder_handle(builder, (Vec2){ .x = 42., .y = 69.});
    vec2_buf_builder_handle(builder, (Vec2){ .x = 0., .y = 69.});
  }
  {
    vec2_buf_builder_handle(builder, (Vec2){ .x = 0., .y = 69.});
    vec2_buf_builder_handle(builder, (Vec2){ .x = 0., .y = 0.});
  }
  vec2_buf_builder_debug_print(builder);
  Vec2Buf buf = vec2_buf_builder_consume(builder);
  for (uint32_t i = 0; i < buf.len; i++) {
    vec2_debug_print(buf.data[i]);
  }
  free(buf.data);
}
