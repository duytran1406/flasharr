// .agent/skills/oceanload-architect/templates/mui-reference/AppShell.jsx

import * as React from 'react';
import { styled, createTheme, ThemeProvider } from '@mui/material/styles';
import CssBaseline from '@mui/material/CssBaseline';
import MuiDrawer from '@mui/material/Drawer';
import Box from '@mui/material/Box';
import MuiAppBar from '@mui/material/AppBar';
import Toolbar from '@mui/material/Toolbar';
import List from '@mui/material/List';
import Typography from '@mui/material/Typography';
import Divider from '@mui/material/Divider';
import IconButton from '@mui/material/IconButton';
import MenuIcon from '@mui/icons-material/Menu';
import ChevronLeftIcon from '@mui/icons-material/ChevronLeft';

// ... (Agent will assume standard drawerWidth logic here)

export default function AppShell({ children }) {
    const [open, setOpen] = React.useState(true);
    const toggleDrawer = () => {
        setOpen(!open);
    };

    return (
        <Box sx={{ display: 'flex' }}>
            <CssBaseline />
            <AppBar position="absolute" open={open}>
                <Toolbar sx={{ pr: '24px' }}>
                    <IconButton edge="start" color="inherit" onClick={toggleDrawer}>
                        <MenuIcon />
                    </IconButton>
                    <Typography component="h1" variant="h6" color="inherit" noWrap sx={{ flexGrow: 1 }}>
                        OceanLoad
                    </Typography>
                </Toolbar>
            </AppBar>
            <Drawer variant="permanent" open={open}>
                <Toolbar>
                    <IconButton onClick={toggleDrawer}><ChevronLeftIcon /></IconButton>
                </Toolbar>
                <Divider />
                <List component="nav">
                    {/* Main List Items (All, Downloading, etc) */}
                </List>
            </Drawer>
            <Box component="main" sx={{ flexGrow: 1, height: '100vh', overflow: 'auto' }}>
                <Toolbar />
                {/* THIS IS WHERE THE CRUD TABLE GOES */}
                {children}
            </Box>
        </Box>
    );
}