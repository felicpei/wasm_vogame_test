const path = require('path');

module.exports = {
  entry: "./index.js",
  output: {
    path: path.resolve(__dirname, "dist"),
    filename: "index.js",
  },
  mode: "development",
 
  //server配置
  devServer: {

    //static配置
    static: {
      directory: path.resolve(__dirname, './'), 
      publicPath: '/'
    },

    //跨域配置
    headers: {
      "Cross-Origin-Embedder-Policy": "require-corp",
      "Cross-Origin-Opener-Policy": "same-origin",
    }
  },
};
