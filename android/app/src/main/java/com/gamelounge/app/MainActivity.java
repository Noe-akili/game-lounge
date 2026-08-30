package com.gamelounge.app;

import android.os.Bundle;
import android.util.Log;
import com.getcapacitor.BridgeActivity;

public class MainActivity extends BridgeActivity {
    @Override
    public void onCreate(Bundle savedInstanceState) {
        registerPlugin(NodeJsPlugin.class);
        super.onCreate(savedInstanceState);
        Log.i("GameLounge", "App started. Server will start via NodeJs plugin.");
    }
}
