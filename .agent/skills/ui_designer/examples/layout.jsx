import React from 'react';
import { Box, AppBar, Toolbar, Fab, Typography, useMediaQuery, useTheme } from '@mui/material';
import AddIcon from '@mui/icons-material/Add';

export default function AppLayout({ children }) {
    const theme = useTheme();
    const isMobile = useMediaQuery(theme.breakpoints.down('md'));

    return (
        <Box sx={{ display: 'flex', minHeight: '100vh', bgcolor: 'background.default', color: 'text.primary' }}>

            {/* Top Bar for Desktop, or generic Header */}
            <AppBar position="fixed" sx={{ zIndex: theme.zIndex.drawer + 1 }}>
                <Toolbar>
                    <Typography variant="h6" noWrap component="div">
                        OceanLoad
                    </Typography>
                </Toolbar>
            </AppBar>

            {/* Main Content */}
            <Box component="main" sx={{ flexGrow: 1, p: 3, mt: 8 }}>
                {children}
            </Box>

            {/* Responsive FAB Placement */}
            <Fab
                color="primary"
                sx={{
                    position: 'fixed',
                    bottom: isMobile ? 20 : 32,
                    right: isMobile ? 'calc(50% - 28px)' : 32, // Centered on mobile, right on desktop
                }}
            >
                <AddIcon />
            </Fab>
        </Box>
    );
}