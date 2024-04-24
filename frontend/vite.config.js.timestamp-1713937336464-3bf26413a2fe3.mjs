// vite.config.js
import { defineConfig } from "file:///root/connect/frontend/node_modules/.pnpm/vite@5.2.10_@types+node@20.12.7_terser@5.30.3/node_modules/vite/dist/node/index.js";
import react from "file:///root/connect/frontend/node_modules/.pnpm/@vitejs+plugin-react@4.2.1_vite@5.2.10_@types+node@20.12.7_terser@5.30.3_/node_modules/@vitejs/plugin-react/dist/index.mjs";
import { VitePWA } from "file:///root/connect/frontend/node_modules/.pnpm/vite-plugin-pwa@0.16.7_vite@5.2.10_@types+node@20.12.7_terser@5.30.3__workbox-build@7.0.0_@ty_7l7hbaje7easw764u4dwv43eba/node_modules/vite-plugin-pwa/dist/index.js";
import svgrPlugin from "file:///root/connect/frontend/node_modules/.pnpm/vite-plugin-svgr@3.3.0_rollup@4.16.1_vite@5.2.10_@types+node@20.12.7_terser@5.30.3_/node_modules/vite-plugin-svgr/dist/index.js";
import { sentryVitePlugin } from "file:///root/connect/frontend/node_modules/.pnpm/@sentry+vite-plugin@2.16.1_encoding@0.1.13/node_modules/@sentry/vite-plugin/dist/esm/index.mjs";
var vite_config_default = defineConfig(({ mode }) => {
  let sentryPlugin;
  if (mode === "production" && process.env.SENTRY_AUTH_TOKEN) {
    sentryPlugin = sentryVitePlugin({
      authToken: process.env.SENTRY_AUTH_TOKEN,
      org: "moretore",
      project: "connect",
      sourcemaps: {
        filesToDeleteAfterUpload: ["**/*.map"]
      }
    });
  }
  return {
    build: {
      // Required for Sentry
      sourcemap: true
    },
    plugins: [
      // TODO: compression plugin
      react(),
      VitePWA({
        workbox: {
          globPatterns: ["**/*.{js,css,html,png,webp,svg,ico}"],
          // TODO: revisit, throw error during build if too large?
          maximumFileSizeToCacheInBytes: 10 * 1024 * 1024,
          sourcemap: true
        }
      }),
      svgrPlugin(),
      sentryPlugin
    ].filter(Boolean),
    optimizeDeps: {
      esbuildOptions: {
        // Node.js global to browser globalThis
        // Required for Material UI v1
        define: {
          global: "globalThis"
        }
      }
    }
  };
});
export {
  vite_config_default as default
};
//# sourceMappingURL=data:application/json;base64,ewogICJ2ZXJzaW9uIjogMywKICAic291cmNlcyI6IFsidml0ZS5jb25maWcuanMiXSwKICAic291cmNlc0NvbnRlbnQiOiBbImNvbnN0IF9fdml0ZV9pbmplY3RlZF9vcmlnaW5hbF9kaXJuYW1lID0gXCIvcm9vdC9jb25uZWN0L2Zyb250ZW5kXCI7Y29uc3QgX192aXRlX2luamVjdGVkX29yaWdpbmFsX2ZpbGVuYW1lID0gXCIvcm9vdC9jb25uZWN0L2Zyb250ZW5kL3ZpdGUuY29uZmlnLmpzXCI7Y29uc3QgX192aXRlX2luamVjdGVkX29yaWdpbmFsX2ltcG9ydF9tZXRhX3VybCA9IFwiZmlsZTovLy9yb290L2Nvbm5lY3QvZnJvbnRlbmQvdml0ZS5jb25maWcuanNcIjtpbXBvcnQgeyBkZWZpbmVDb25maWcgfSBmcm9tICd2aXRlJztcbmltcG9ydCByZWFjdCBmcm9tICdAdml0ZWpzL3BsdWdpbi1yZWFjdCc7XG5pbXBvcnQgeyBWaXRlUFdBIH0gZnJvbSAndml0ZS1wbHVnaW4tcHdhJztcbmltcG9ydCBzdmdyUGx1Z2luIGZyb20gJ3ZpdGUtcGx1Z2luLXN2Z3InO1xuaW1wb3J0IHsgc2VudHJ5Vml0ZVBsdWdpbiB9IGZyb20gJ0BzZW50cnkvdml0ZS1wbHVnaW4nO1xuXG4vLyBodHRwczovL3ZpdGVqcy5kZXYvY29uZmlnL1xuZXhwb3J0IGRlZmF1bHQgZGVmaW5lQ29uZmlnKCh7IG1vZGUgfSkgPT4ge1xuICBsZXQgc2VudHJ5UGx1Z2luO1xuICBpZiAobW9kZSA9PT0gJ3Byb2R1Y3Rpb24nICYmIHByb2Nlc3MuZW52LlNFTlRSWV9BVVRIX1RPS0VOKSB7XG4gICAgc2VudHJ5UGx1Z2luID0gc2VudHJ5Vml0ZVBsdWdpbih7XG4gICAgICBhdXRoVG9rZW46IHByb2Nlc3MuZW52LlNFTlRSWV9BVVRIX1RPS0VOLFxuICAgICAgb3JnOiAnbW9yZXRvcmUnLFxuICAgICAgcHJvamVjdDogJ2Nvbm5lY3QnLFxuICAgICAgc291cmNlbWFwczoge1xuICAgICAgICBmaWxlc1RvRGVsZXRlQWZ0ZXJVcGxvYWQ6IFsnKiovKi5tYXAnXSxcbiAgICAgIH0sXG4gICAgfSk7XG4gIH1cblxuICByZXR1cm4ge1xuICAgIGJ1aWxkOiB7XG4gICAgICAvLyBSZXF1aXJlZCBmb3IgU2VudHJ5XG4gICAgICBzb3VyY2VtYXA6IHRydWUsXG4gICAgfSxcbiAgICBwbHVnaW5zOiBbXG4gICAgICAvLyBUT0RPOiBjb21wcmVzc2lvbiBwbHVnaW5cbiAgICAgIHJlYWN0KCksXG4gICAgICBWaXRlUFdBKHtcbiAgICAgICAgd29ya2JveDoge1xuICAgICAgICAgIGdsb2JQYXR0ZXJuczogWycqKi8qLntqcyxjc3MsaHRtbCxwbmcsd2VicCxzdmcsaWNvfSddLFxuICAgICAgICAgIC8vIFRPRE86IHJldmlzaXQsIHRocm93IGVycm9yIGR1cmluZyBidWlsZCBpZiB0b28gbGFyZ2U/XG4gICAgICAgICAgbWF4aW11bUZpbGVTaXplVG9DYWNoZUluQnl0ZXM6IDEwICogMTAyNCAqIDEwMjQsXG4gICAgICAgICAgc291cmNlbWFwOiB0cnVlLFxuICAgICAgICB9LFxuICAgICAgfSksXG4gICAgICBzdmdyUGx1Z2luKCksXG4gICAgICBzZW50cnlQbHVnaW4sXG4gICAgXS5maWx0ZXIoQm9vbGVhbiksXG4gICAgb3B0aW1pemVEZXBzOiB7XG4gICAgICBlc2J1aWxkT3B0aW9uczoge1xuICAgICAgICAvLyBOb2RlLmpzIGdsb2JhbCB0byBicm93c2VyIGdsb2JhbFRoaXNcbiAgICAgICAgLy8gUmVxdWlyZWQgZm9yIE1hdGVyaWFsIFVJIHYxXG4gICAgICAgIGRlZmluZToge1xuICAgICAgICAgIGdsb2JhbDogJ2dsb2JhbFRoaXMnLFxuICAgICAgICB9LFxuICAgICAgfSxcbiAgICB9LFxuICB9O1xufSk7XG4iXSwKICAibWFwcGluZ3MiOiAiO0FBQW9QLFNBQVMsb0JBQW9CO0FBQ2pSLE9BQU8sV0FBVztBQUNsQixTQUFTLGVBQWU7QUFDeEIsT0FBTyxnQkFBZ0I7QUFDdkIsU0FBUyx3QkFBd0I7QUFHakMsSUFBTyxzQkFBUSxhQUFhLENBQUMsRUFBRSxLQUFLLE1BQU07QUFDeEMsTUFBSTtBQUNKLE1BQUksU0FBUyxnQkFBZ0IsUUFBUSxJQUFJLG1CQUFtQjtBQUMxRCxtQkFBZSxpQkFBaUI7QUFBQSxNQUM5QixXQUFXLFFBQVEsSUFBSTtBQUFBLE1BQ3ZCLEtBQUs7QUFBQSxNQUNMLFNBQVM7QUFBQSxNQUNULFlBQVk7QUFBQSxRQUNWLDBCQUEwQixDQUFDLFVBQVU7QUFBQSxNQUN2QztBQUFBLElBQ0YsQ0FBQztBQUFBLEVBQ0g7QUFFQSxTQUFPO0FBQUEsSUFDTCxPQUFPO0FBQUE7QUFBQSxNQUVMLFdBQVc7QUFBQSxJQUNiO0FBQUEsSUFDQSxTQUFTO0FBQUE7QUFBQSxNQUVQLE1BQU07QUFBQSxNQUNOLFFBQVE7QUFBQSxRQUNOLFNBQVM7QUFBQSxVQUNQLGNBQWMsQ0FBQyxxQ0FBcUM7QUFBQTtBQUFBLFVBRXBELCtCQUErQixLQUFLLE9BQU87QUFBQSxVQUMzQyxXQUFXO0FBQUEsUUFDYjtBQUFBLE1BQ0YsQ0FBQztBQUFBLE1BQ0QsV0FBVztBQUFBLE1BQ1g7QUFBQSxJQUNGLEVBQUUsT0FBTyxPQUFPO0FBQUEsSUFDaEIsY0FBYztBQUFBLE1BQ1osZ0JBQWdCO0FBQUE7QUFBQTtBQUFBLFFBR2QsUUFBUTtBQUFBLFVBQ04sUUFBUTtBQUFBLFFBQ1Y7QUFBQSxNQUNGO0FBQUEsSUFDRjtBQUFBLEVBQ0Y7QUFDRixDQUFDOyIsCiAgIm5hbWVzIjogW10KfQo=
