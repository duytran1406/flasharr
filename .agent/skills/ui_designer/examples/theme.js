import { createTheme } from '@mui/material/styles';

// Design Tokens - DO NOT CHANGE
const fshareRed = '#D32F2F';
const oceanDark = '#0F1C24';
const oceanPaper = '#182936';
const neonTeal = '#1DE9B6';

export const lightTheme = createTheme({
    palette: {
        mode: 'light',
        primary: { main: fshareRed, contrastText: '#ffffff' },
        secondary: { main: '#FF5252' },
        background: { default: '#F4F6F8', paper: '#FFFFFF' },
        text: { primary: '#212121', secondary: '#757575' },
    },
    components: {
        MuiAppBar: { styleOverrides: { root: { backgroundColor: fshareRed } } },
        MuiFab: { styleOverrides: { root: { backgroundColor: fshareRed } } },
    },
});

export const darkTheme = createTheme({
    palette: {
        mode: 'dark',
        primary: { main: neonTeal, contrastText: '#000000' },
        secondary: { main: '#00BFA5' },
        background: { default: oceanDark, paper: oceanPaper },
        text: { primary: '#E0F7FA', secondary: '#90A4AE' },
        action: { active: neonTeal },
    },
    components: {
        MuiAppBar: {
            styleOverrides: {
                root: {
                    backgroundColor: oceanPaper,
                    backgroundImage: 'none',
                    borderBottom: `1px solid rgba(29, 233, 182, 0.1)`,
                },
            },
        },
        MuiPaper: { styleOverrides: { root: { backgroundImage: 'none' } } },
    },
});