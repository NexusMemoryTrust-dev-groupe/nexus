# Nexus Benchmark — real measurements

- Projects dir: `..\benchmarks\retrieval\projects`
- Files indexed: **1320**
- Embedding: real ONNX all-MiniLM-L6-v2 (FASTEMBED_CACHE_DIR)
- Token counting: tiktoken `gpt-4o` (exact, offline)

- ONNX model loaded: **true**

- Indexing: 1320/1320 files in 334.7s (4 files/s)

- Co-location graph: **1320** entities, **1058** files with cached neighbours

- Retrieval cases: **118**
## Retrieval benchmark (hybrid vs keyword)

| Query | Sem P@5 | Sem R@5 | Sem R@20 | KW P@5 | KW R@5 | KW R@20 | Rel |
|---|---|---|---|---|---|---|---|
| How are HTTP sessions and cookies manage… | 0.40 | 1.00 | 1.00 | 0.20 | 0.50 | 1.00 | 2 |
| What authentication schemes are supporte… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 1.00 | 1 |
| How does the library retry failed HTTP r… | 0.20 | 0.33 | 0.67 | 0.20 | 0.33 | 0.33 | 3 |
| How are query string parameters encoded … | 0.40 | 0.40 | 0.60 | 0.40 | 0.40 | 0.40 | 5 |
| How does the requests library expose the… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How is the Response object implemented i… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 1.00 | 1 |
| How does HTTPAdapter manage connection p… | 0.20 | 0.50 | 1.00 | 0.20 | 0.50 | 0.50 | 2 |
| What exceptions does requests define and… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How does the hooks dispatch system work? | 0.40 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 2 |
| How is the case-insensitive dictionary i… | 0.40 | 1.00 | 1.00 | 0.20 | 0.50 | 0.50 | 2 |
| How are cookies merged into the session … | 0.40 | 1.00 | 1.00 | 0.40 | 1.00 | 1.00 | 2 |
| How does Session follow redirects across… | 0.40 | 1.00 | 1.00 | 0.20 | 0.50 | 1.00 | 2 |
| How are connect and read timeouts enforc… | 0.20 | 0.33 | 0.67 | 0.40 | 0.67 | 0.67 | 3 |
| How does TLS certificate verification wo… | 0.20 | 0.33 | 0.67 | 0.20 | 0.33 | 0.33 | 3 |
| How are multipart file uploads encoded? | 0.40 | 0.40 | 0.60 | 0.20 | 0.20 | 0.60 | 5 |
| How is proxy support implemented in sess… | 0.40 | 0.67 | 0.67 | 0.20 | 0.33 | 0.67 | 3 |
| How are status codes exposed as constant… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How does the compat module bridge Python… | 0.40 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 2 |
| How is the help module used to introspec… | 0.20 | 0.50 | 1.00 | 0.00 | 0.00 | 0.00 | 2 |
| How are cookies extracted from HTTP resp… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How does the library handle content deco… | 0.20 | 0.33 | 0.33 | 0.20 | 0.33 | 0.33 | 3 |
| How is the session-level authentication … | 0.40 | 1.00 | 1.00 | 0.20 | 0.50 | 1.00 | 2 |
| How does prepare_request build a Prepare… | 0.40 | 1.00 | 1.00 | 0.20 | 0.50 | 1.00 | 2 |
| How is the vendor packages shim implemen… | 0.20 | 0.50 | 0.50 | 0.00 | 0.00 | 0.00 | 2 |
| How does the library detect and handle c… | 0.20 | 1.00 | 1.00 | 0.20 | 1.00 | 1.00 | 1 |
| How is the logger initialized and config… | 0.40 | 0.67 | 1.00 | 0.20 | 0.33 | 0.67 | 3 |
| Where are log levels and maximum level f… | 0.20 | 1.00 | 1.00 | 0.20 | 1.00 | 1.00 | 1 |
| How are the log macros (info!, warn!, er… | 0.40 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 2 |
| How does the serde feature serialize log… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 1.00 | 1 |
| How is the private API module structured… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 1.00 | 1 |
| How are log errors represented? | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How is key-value logging supported in lo… | 0.60 | 1.00 | 1.00 | 0.00 | 0.00 | 0.33 | 3 |
| How is the source location (file, line, … | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How does the Log trait define the loggin… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 1.00 | 1 |
| How is the Record type constructed and w… | 0.20 | 1.00 | 1.00 | 0.20 | 1.00 | 1.00 | 1 |
| How is the module tree organized in log? | 0.40 | 1.00 | 1.00 | 0.20 | 0.50 | 1.00 | 2 |
| How is the maximum level filter updated … | 0.20 | 1.00 | 1.00 | 0.20 | 1.00 | 1.00 | 1 |
| How are structured values formatted for … | 0.40 | 1.00 | 1.00 | 0.20 | 0.50 | 0.50 | 2 |
| How is the MUI Button component implemen… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 1.00 | 1 |
| How does the MUI Dialog component handle… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 1.00 | 1 |
| How does MUI Checkbox support the indete… | 0.20 | 1.00 | 1.00 | 0.20 | 1.00 | 1.00 | 1 |
| How is the MUI Accordion component imple… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 1.00 | 1 |
| How does MUI AccordionSummary render the… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How is the MUI Alert component styled wi… | 0.20 | 1.00 | 1.00 | 0.20 | 1.00 | 1.00 | 1 |
| How is the MUI AppBar component implemen… | 0.20 | 1.00 | 1.00 | 0.20 | 1.00 | 1.00 | 1 |
| How does MUI Autocomplete filter and sel… | 0.20 | 1.00 | 1.00 | 0.20 | 1.00 | 1.00 | 1 |
| How is the MUI Avatar component implemen… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How does MUI AvatarGroup stack multiple … | 0.20 | 1.00 | 1.00 | 0.20 | 1.00 | 1.00 | 1 |
| How is the MUI Badge component implement… | 0.20 | 1.00 | 1.00 | 0.20 | 1.00 | 1.00 | 1 |
| How is the MUI BottomNavigation componen… | 0.20 | 1.00 | 1.00 | 0.20 | 1.00 | 1.00 | 1 |
| How is the MUI Box component implemented… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How does MUI Breadcrumbs render navigati… | 0.20 | 1.00 | 1.00 | 0.20 | 1.00 | 1.00 | 1 |
| How is the MUI ButtonGroup component imp… | 0.20 | 1.00 | 1.00 | 0.20 | 1.00 | 1.00 | 1 |
| How is the MUI ButtonBase component impl… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How is the MUI Card component implemente… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How does MUI CardMedia render media cont… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How is the MUI Chip component implemente… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 1.00 | 1 |
| How does MUI CircularProgress animate lo… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 1.00 | 1 |
| How is the MUI Collapse component implem… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How is the MUI Container component imple… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How is the MUI Divider component impleme… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How is the MUI Drawer component implemen… | 0.20 | 1.00 | 1.00 | 0.20 | 1.00 | 1.00 | 1 |
| How is the MUI Fab component implemented… | 0.20 | 1.00 | 1.00 | 0.20 | 1.00 | 1.00 | 1 |
| How does MUI FormControl provide context… | 0.20 | 1.00 | 1.00 | 0.20 | 1.00 | 1.00 | 1 |
| How does MUI FormControlLabel render lab… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How is the MUI FormGroup component imple… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How does MUI FormHelperText render helpe… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How is the MUI Grid component implemente… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How is the MUI Icon component implemente… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How is the MUI IconButton component impl… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 1.00 | 1 |
| How is the MUI ImageList component imple… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How does MUI Input handle text input sty… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How is MUI InputAdornment implemented? | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How is MUI InputBase implemented? | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 1.00 | 1 |
| How does MUI InputLabel float labels for… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How is the MUI LinearProgress component … | 0.20 | 1.00 | 1.00 | 0.20 | 1.00 | 1.00 | 1 |
| How is the MUI Link component implemente… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How is the MUI List component implemente… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How does MUI ListItem render list items? | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How is MUI ListItemButton implemented? | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How does MUI ListItemIcon render icons i… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How is MUI ListItemText implemented? | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How is the MUI ListSubheader component i… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How does the MUI Menu component work? | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How is the MUI MenuItem component implem… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How is the MUI MenuList component implem… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How is the MUI Modal component implement… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 1.00 | 1 |
| How does MUI NativeSelect render native … | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 1.00 | 1 |
| How is MUI OutlinedInput implemented? | 0.20 | 1.00 | 1.00 | 0.20 | 1.00 | 1.00 | 1 |
| How is the MUI Pagination component impl… | 0.20 | 1.00 | 1.00 | 0.20 | 1.00 | 1.00 | 1 |
| How is the MUI Paper component implement… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How does the MUI Popover component work? | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 1.00 | 1 |
| How is the MUI Popper component implemen… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 1.00 | 1 |
| How is the MUI Radio component implement… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 1.00 | 1 |
| How is the MUI Rating component implemen… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How does MUI Select render dropdown sele… | 0.20 | 1.00 | 1.00 | 0.20 | 1.00 | 1.00 | 1 |
| How is the MUI Skeleton component implem… | 0.20 | 1.00 | 1.00 | 0.20 | 1.00 | 1.00 | 1 |
| How does MUI Slider handle range input? | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 1.00 | 1 |
| How is the MUI Snackbar component implem… | 0.20 | 1.00 | 1.00 | 0.20 | 1.00 | 1.00 | 1 |
| How is the MUI Stack component implement… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How is the MUI Stepper component impleme… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How is the MUI Switch component implemen… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 1.00 | 1 |
| How is the MUI Tab component implemented… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How is the MUI Table component implement… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How does MUI TableCell render table cell… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How does MUI TablePagination implement p… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How is MUI TableSortLabel implemented? | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How is the MUI Tabs component implemente… | 0.20 | 1.00 | 1.00 | 0.20 | 1.00 | 1.00 | 1 |
| How is the MUI TextField component imple… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 1.00 | 1 |
| How is the MUI ToggleButton component im… | 0.20 | 1.00 | 1.00 | 0.20 | 1.00 | 1.00 | 1 |
| How is the MUI Toolbar component impleme… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How does the MUI Tooltip component work? | 0.20 | 1.00 | 1.00 | 0.20 | 1.00 | 1.00 | 1 |
| How is the MUI Typography component impl… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How is the MUI Zoom component implemente… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How is the MUI ListItemAvatar component … | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How is the MUI ListItemSecondaryAction c… | 0.20 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 | 1 |
| How is the MUI SpeedDial component imple… | 0.20 | 1.00 | 1.00 | 0.20 | 1.00 | 1.00 | 1 |
| How is the MUI SwipeableDrawer component… | 0.20 | 1.00 | 1.00 | 0.20 | 1.00 | 1.00 | 1 |
| **Mean (118)** | **0.23** | **0.95** | **0.97** | **0.08** | **0.30** | **0.50** | — |
| **Mean MUI (80)** | — | **1.00** | **1.00** | — | — | — | — |

## Phase 1.2/1.3 — graph expansion + reranker

| Query | Hybrid R@20 | Exp R@5 | Exp R@20 | Reranked P@5 | Rel |
|---|---|---|---|---|---|
| How are HTTP sessions and cookies manage… | 1.00 | 1.00 | 1.00 | 0.40 | 2 |
| What authentication schemes are supporte… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How does the library retry failed HTTP r… | 0.67 | 0.33 | 0.33 | 0.40 | 3 |
| How are query string parameters encoded … | 0.60 | 0.40 | 0.40 | 0.40 | 5 |
| How does the requests library expose the… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the Response object implemented i… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How does HTTPAdapter manage connection p… | 1.00 | 0.50 | 0.50 | 0.20 | 2 |
| What exceptions does requests define and… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How does the hooks dispatch system work? | 1.00 | 0.50 | 1.00 | 0.40 | 2 |
| How is the case-insensitive dictionary i… | 1.00 | 0.50 | 1.00 | 0.20 | 2 |
| How are cookies merged into the session … | 1.00 | 1.00 | 1.00 | 0.40 | 2 |
| How does Session follow redirects across… | 1.00 | 1.00 | 1.00 | 0.40 | 2 |
| How are connect and read timeouts enforc… | 0.67 | 0.33 | 0.67 | 0.40 | 3 |
| How does TLS certificate verification wo… | 0.67 | 0.33 | 0.67 | 0.40 | 3 |
| How are multipart file uploads encoded? | 0.60 | 0.20 | 0.60 | 0.40 | 5 |
| How is proxy support implemented in sess… | 0.67 | 0.33 | 0.67 | 0.40 | 3 |
| How are status codes exposed as constant… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How does the compat module bridge Python… | 1.00 | 1.00 | 1.00 | 0.20 | 2 |
| How is the help module used to introspec… | 1.00 | 0.50 | 0.50 | 0.20 | 2 |
| How are cookies extracted from HTTP resp… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How does the library handle content deco… | 0.33 | 0.33 | 0.67 | 0.20 | 3 |
| How is the session-level authentication … | 1.00 | 1.00 | 1.00 | 0.40 | 2 |
| How does prepare_request build a Prepare… | 1.00 | 1.00 | 1.00 | 0.40 | 2 |
| How is the vendor packages shim implemen… | 0.50 | 0.00 | 0.50 | 0.20 | 2 |
| How does the library detect and handle c… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the logger initialized and config… | 1.00 | 0.67 | 1.00 | 0.40 | 3 |
| Where are log levels and maximum level f… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How are the log macros (info!, warn!, er… | 1.00 | 1.00 | 1.00 | 0.40 | 2 |
| How does the serde feature serialize log… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the private API module structured… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How are log errors represented? | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is key-value logging supported in lo… | 1.00 | 1.00 | 1.00 | 0.60 | 3 |
| How is the source location (file, line, … | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How does the Log trait define the loggin… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the Record type constructed and w… | 1.00 | 0.00 | 1.00 | 0.00 | 1 |
| How is the module tree organized in log? | 1.00 | 0.50 | 1.00 | 0.40 | 2 |
| How is the maximum level filter updated … | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How are structured values formatted for … | 1.00 | 0.50 | 1.00 | 0.20 | 2 |
| How is the MUI Button component implemen… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How does the MUI Dialog component handle… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How does MUI Checkbox support the indete… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI Accordion component imple… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How does MUI AccordionSummary render the… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI Alert component styled wi… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI AppBar component implemen… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How does MUI Autocomplete filter and sel… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI Avatar component implemen… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How does MUI AvatarGroup stack multiple … | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI Badge component implement… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI BottomNavigation componen… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI Box component implemented… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How does MUI Breadcrumbs render navigati… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI ButtonGroup component imp… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI ButtonBase component impl… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI Card component implemente… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How does MUI CardMedia render media cont… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI Chip component implemente… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How does MUI CircularProgress animate lo… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI Collapse component implem… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI Container component imple… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI Divider component impleme… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI Drawer component implemen… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI Fab component implemented… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How does MUI FormControl provide context… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How does MUI FormControlLabel render lab… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI FormGroup component imple… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How does MUI FormHelperText render helpe… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI Grid component implemente… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI Icon component implemente… | 1.00 | 1.00 | 1.00 | 0.00 | 1 |
| How is the MUI IconButton component impl… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI ImageList component imple… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How does MUI Input handle text input sty… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is MUI InputAdornment implemented? | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is MUI InputBase implemented? | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How does MUI InputLabel float labels for… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI LinearProgress component … | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI Link component implemente… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI List component implemente… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How does MUI ListItem render list items? | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is MUI ListItemButton implemented? | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How does MUI ListItemIcon render icons i… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is MUI ListItemText implemented? | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI ListSubheader component i… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How does the MUI Menu component work? | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI MenuItem component implem… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI MenuList component implem… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI Modal component implement… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How does MUI NativeSelect render native … | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is MUI OutlinedInput implemented? | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI Pagination component impl… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI Paper component implement… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How does the MUI Popover component work? | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI Popper component implemen… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI Radio component implement… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI Rating component implemen… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How does MUI Select render dropdown sele… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI Skeleton component implem… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How does MUI Slider handle range input? | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI Snackbar component implem… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI Stack component implement… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI Stepper component impleme… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI Switch component implemen… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI Tab component implemented… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI Table component implement… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How does MUI TableCell render table cell… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How does MUI TablePagination implement p… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is MUI TableSortLabel implemented? | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI Tabs component implemente… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI TextField component imple… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI ToggleButton component im… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI Toolbar component impleme… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How does the MUI Tooltip component work? | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI Typography component impl… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI Zoom component implemente… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI ListItemAvatar component … | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI ListItemSecondaryAction c… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI SpeedDial component imple… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| How is the MUI SwipeableDrawer component… | 1.00 | 1.00 | 1.00 | 0.20 | 1 |
| **Mean (118)** | — | **0.91** | **0.96** | **0.22** | — |
| **Mean MUI (80)** | — | **1.00** | **1.00** | — | — |

## Per-dataset retrieval evidence — before vs after reranker

| Dataset | Queries | P@5 | R@5 | MRR@10 | NDCG@10 | (before reranker) |
|---|---|---|---|---|---|---|
| requests | 25 | 0.29 | 0.77 | 0.88 | 0.80 | |
| rust-log | 13 | 0.29 | 0.97 | 0.83 | 0.85 | |
| mui | 80 | 0.20 | 1.00 | 0.99 | 0.99 | |

| Dataset | Queries | P@5 | R@5 | MRR@10 | NDCG@10 | (after reranker) |
|---|---|---|---|---|---|---|
| requests | 25 | 0.30 | 0.77 | 0.96 | 0.81 | |
| rust-log | 13 | 0.26 | 0.86 | 0.86 | 0.83 | |
| mui | 80 | 0.20 | 0.99 | 0.97 | 0.98 | |

| Dataset | ΔP@5 | ΔR@5 | ΔMRR@10 | ΔNDCG@10 | (after − before) |
|---|---|---|---|---|---|
| requests | +0.01 | -0.00 | +0.08 | +0.01 | |
| rust-log | -0.03 | -0.12 | +0.03 | -0.02 | |
| mui | -0.00 | -0.01 | -0.01 | -0.01 | |

**Interpretation.** Чанкинг длинных файлов (1024 B, overlap 128) закрыл единственный семантический miss: `Record` в `rust-log/src/lib.rs` (позиция >8192 B) теперь находится на #4 в hybrid (R@5 0.00 → 1.00), missing rate упал с 0.0085 до **0.0000**. Оба канала выиграли от полного `source_text` (64 KB): semantic R@5 0.91 → 0.95, keyword R@5 0.20 → 0.30, R@20 0.34 → 0.50. Reranker по-прежнему не панацея: 2 демоушна из top-5 (Record #4→#7, Icon #1→#28) против 0 спасений, нетто-эффект на распределении бакетов: top-1 107→110. Токен-экономика улучшилась: reduction 77.4% → **94.2%**, средняя задержка 5640.3 → **1863.1 ms**.

## Failure analysis — where retrieval still misses

Bucket of the first relevant hit (118 queries, hybrid top-50 pool):

| Bucket | Before (hybrid) | After (reranked) | Δ |
|---|---|---|---|
| top-1 | 107 | 110 | +3 |
| top-2-3 | 9 | 6 | -3 |
| top-4-5 | 2 | 0 | -2 |
| top-6-10 | 0 | 1 | +1 |
| top-11-50 | 0 | 1 | +1 |
| missing | 0 | 0 | +0 |

- wrong top-1 (after): **8** (7%)
- wrong top-3 (after): **2** (2%)
- wrong top-5 (after): **2** (2%)
- missing from the top-50 pool (after): **0** (0%)

### Classification of misses (first relevant not in top-5 after rerank)

| Dataset | Query | Rank before → after | Class | Evidence |
|---|---|---|---|---|
| rust-log | "How is the Record type constructed and…" | #4 → #7 | reranker demotion | hybrid had it at #4, rerank lost it |
| mui | "How is the MUI Icon component implemen…" | #1 → #28 | reranker demotion | hybrid had it at #1, rerank lost it |

- **2** cases with the first relevant hit outside top-5 (after rerank)
- Reranker demotions: 2 (relevant was top-5 in hybrid order)
- Reranker rescues: 0 (relevant moved INTO top-5 by rerank)
- Graph-pool recoveries: 0 (relevant found only after expansion)
- Classified misses: 0 semantic, 0 lexical, 0 path/symbol, 0 graph, 2 reranker, 0 ground-truth ambiguity, 0 ground-truth miss

NEXUS_METRIC retr_after_top1_rate 0.9322
NEXUS_METRIC retr_after_top5_rate 0.9831
NEXUS_METRIC retr_missing_rate 0.0000
NEXUS_METRIC retr_reranker_demotions 2
NEXUS_METRIC retr_reranker_rescues 0
NEXUS_METRIC retr_graph_recoveries 0

### What Nexus hybrid search actually returned (top 5)

- "How are HTTP sessions and cookies managed in…" → `requests/src\requests\sessions.py` (0.708), `requests/src\requests\cookies.py` (0.679), `requests/HISTORY.md` (0.392), `requests/tests\test_requests.py` (0.381), `requests/src\requests\models.py` (0.328)
- "What authentication schemes are supported (b…" → `requests/src\requests\auth.py` (0.388), `requests/HISTORY.md` (0.367), `requests/src\requests\utils.py` (0.285), `requests/src\requests\models.py` (0.260), `requests/src\requests\sessions.py` (0.229)
- "How does the library retry failed HTTP reque…" → `requests/HISTORY.md` (0.406), `requests/src\requests\adapters.py` (0.382), `requests/tests\test_requests.py` (0.342), `rust-log/src\lib.rs` (0.342), `requests/src\requests\models.py` (0.336)
- "How are query string parameters encoded into…" → `requests/HISTORY.md` (0.358), `requests/src\requests\models.py` (0.348), `requests/src\requests\utils.py` (0.333), `requests/src\requests\sessions.py` (0.279), `requests/tests\test_requests.py` (0.275)
- "How does the requests library expose the pub…" → `requests/src\requests\api.py` (0.487), `requests/HISTORY.md` (0.360), `rust-log/src\lib.rs` (0.310), `requests/src\requests\_types.py` (0.294), `requests/src\requests\models.py` (0.272)
- "How is the Response object implemented in re…" → `requests/src\requests\models.py` (0.432), `requests/src\requests\cookies.py` (0.403), `requests/src\requests\adapters.py` (0.398), `requests/src\requests\auth.py` (0.384), `requests/src\requests\sessions.py` (0.361)
- "How does HTTPAdapter manage connection pools…" → `requests/src\requests\adapters.py` (0.377), `requests/HISTORY.md` (0.317), `requests/tests\test_requests.py` (0.236), `requests/src\requests\sessions.py` (0.221), `requests/src\requests\models.py` (0.207)
- "What exceptions does requests define and whe…" → `requests/src\requests\exceptions.py` (0.548), `requests/HISTORY.md` (0.394), `requests/src\requests\sessions.py` (0.368), `requests/tests\test_requests.py` (0.351), `requests/src\requests\adapters.py` (0.314)
- "How does the hooks dispatch system work?" → `requests/src\requests\hooks.py` (0.626), `requests/tests\test_hooks.py` (0.332), `requests/HISTORY.md` (0.267), `requests/src\requests\sessions.py` (0.230), `requests/src\requests\models.py` (0.184)
- "How is the case-insensitive dictionary imple…" → `requests/src\requests\structures.py` (0.330), `requests/src\requests\models.py` (0.275), `requests/src\requests\adapters.py` (0.259), `requests/src\requests\utils.py` (0.249), `requests/tests\test_structures.py` (0.224)
- "How are cookies merged into the session cook…" → `requests/src\requests\cookies.py` (0.721), `requests/src\requests\sessions.py` (0.441), `requests/tests\test_requests.py` (0.335), `requests/src\requests\models.py` (0.311), `requests/HISTORY.md` (0.306)
- "How does Session follow redirects across req…" → `requests/src\requests\sessions.py` (0.543), `requests/HISTORY.md` (0.403), `requests/src\requests\models.py` (0.367), `requests/tests\test_requests.py` (0.334), `requests/tests\test_lowlevel.py` (0.292)
- "How are connect and read timeouts enforced?" → `requests/README.md` (0.295), `requests/src\requests\adapters.py` (0.267), `requests/HISTORY.md` (0.256), `mui/packages\mui-material\src\internal\Transition.tsx` (0.244), `mui/packages\mui-material\src\internal\Transition.test.tsx` (0.235)
- "How does TLS certificate verification work?" → `requests/tests\certs\mtls\README.md` (0.352), `requests/src\requests\sessions.py` (0.312), `requests/src\requests\adapters.py` (0.279), `requests/HISTORY.md` (0.276), `requests/tests\test_requests.py` (0.242)
- "How are multipart file uploads encoded?" → `requests/src\requests\models.py` (0.359), `requests/HISTORY.md` (0.312), `requests/tests\test_utils.py` (0.260), `requests/tests\test_requests.py` (0.248), `requests/src\requests\sessions.py` (0.236)
- "How is proxy support implemented in sessions…" → `requests/src\requests\sessions.py` (0.608), `requests/src\requests\adapters.py` (0.320), `requests/HISTORY.md` (0.304), `requests/src\requests\cookies.py` (0.261), `requests/src\requests\auth.py` (0.247)
- "How are status codes exposed as constants?" → `requests/src\requests\status_codes.py` (0.393), `requests/src\requests\sessions.py` (0.198), `requests/src\requests\models.py` (0.190), `requests/HISTORY.md` (0.175), `mui/CONTRIBUTING.md` (0.155)
- "How does the compat module bridge Python 2 a…" → `requests/src\requests\compat.py` (0.588), `requests/tests\compat.py` (0.517), `requests/HISTORY.md` (0.254), `requests/src\requests\utils.py` (0.249), `requests/src\requests\models.py` (0.226)
- "How is the help module used to introspect th…" → `requests/src\requests\help.py` (0.415), `rust-log/src\kv\mod.rs` (0.228), `mui/CHANGELOG.md` (0.205), `requests/HISTORY.md` (0.203), `rust-log/rfcs\0296-structured-logging.md` (0.201)
- "How are cookies extracted from HTTP response…" → `requests/src\requests\cookies.py` (0.591), `requests/HISTORY.md` (0.323), `requests/src\requests\models.py` (0.309), `requests/src\requests\sessions.py` (0.291), `requests/src\requests\utils.py` (0.278)
- "How does the library handle content decoding…" → `requests/src\requests\models.py` (0.306), `requests/HISTORY.md` (0.301), `rust-log/src\lib.rs` (0.212), `requests/README.md` (0.202), `rust-log/rfcs\0296-structured-logging.md` (0.191)
- "How is the session-level authentication hook…" → `requests/src\requests\sessions.py` (0.341), `requests/src\requests\auth.py` (0.329), `requests/HISTORY.md` (0.269), `requests/src\requests\models.py` (0.230), `requests/tests\test_lowlevel.py` (0.224)
- "How does prepare_request build a PreparedReq…" → `requests/src\requests\sessions.py` (0.360), `requests/tests\test_requests.py` (0.330), `requests/HISTORY.md` (0.317), `requests/src\requests\models.py` (0.278), `requests/src\requests\adapters.py` (0.246)
- "How is the vendor packages shim implemented?" → `rust-log/rfcs\0296-structured-logging.md` (0.207), `requests/HISTORY.md` (0.174), `mui/CHANGELOG.md` (0.161), `rust-log/CHANGELOG.md` (0.159), `requests/src\requests\packages.py` (0.146)
- "How does the library detect and handle conte…" → `requests/src\requests\models.py` (0.334), `requests/HISTORY.md` (0.290), `mui/packages\mui-material\src\utils\types.ts` (0.267), `mui/packages\mui-material\src\transitions\types.ts` (0.253), `requests/src\requests\_types.py` (0.240)
- "How is the logger initialized and configured…" → `rust-log/src\lib.rs` (0.551), `rust-log/CHANGELOG.md` (0.448), `rust-log/README.md` (0.429), `rust-log/rfcs\0296-structured-logging.md` (0.412), `rust-log/src\macros.rs` (0.408)
- "Where are log levels and maximum level filte…" → `rust-log/src\lib.rs` (0.416), `rust-log/test_max_level_features\main.rs` (0.327), `rust-log/CHANGELOG.md` (0.305), `rust-log/test_max_level_features\Cargo.toml` (0.303), `rust-log/rfcs\0296-structured-logging.md` (0.293)
- "How are the log macros (info!, warn!, error!…" → `rust-log/src\macros.rs` (0.660), `rust-log/tests\macros.rs` (0.655), `rust-log/src\kv\error.rs` (0.485), `rust-log/CHANGELOG.md` (0.452), `rust-log/rfcs\0296-structured-logging.md` (0.410)
- "How does the serde feature serialize log rec…" → `rust-log/src\serde.rs` (0.673), `rust-log/rfcs\0296-structured-logging.md` (0.466), `rust-log/src\kv\mod.rs` (0.423), `rust-log/src\lib.rs` (0.404), `rust-log/CHANGELOG.md` (0.364)
- "How is the private API module structured in …" → `rust-log/src\__private_api.rs` (0.544), `rust-log/rfcs\0296-structured-logging.md` (0.489), `requests/src\requests\api.py` (0.461), `rust-log/src\kv\mod.rs` (0.450), `rust-log/src\lib.rs` (0.425)
- "How are log errors represented?" → `rust-log/rfcs\0296-structured-logging.md` (0.480), `rust-log/src\kv\error.rs` (0.448), `rust-log/src\lib.rs` (0.372), `rust-log/src\macros.rs` (0.367), `rust-log/CHANGELOG.md` (0.366)
- "How is key-value logging supported in log?" → `rust-log/src\kv\value.rs` (0.739), `rust-log/src\kv\key.rs` (0.692), `rust-log/benches\value.rs` (0.660), `rust-log/rfcs\0296-structured-logging.md` (0.590), `rust-log/CHANGELOG.md` (0.546)
- "How is the source location (file, line, modu…" → `rust-log/src\kv\source.rs` (0.366), `rust-log/src\lib.rs` (0.322), `rust-log/rfcs\0296-structured-logging.md` (0.270), `rust-log/src\kv\mod.rs` (0.250), `rust-log/CHANGELOG.md` (0.240)
- "How does the Log trait define the logging in…" → `rust-log/rfcs\0296-structured-logging.md` (0.556), `rust-log/src\lib.rs` (0.507), `rust-log/src\kv\mod.rs` (0.490), `rust-log/src\macros.rs` (0.477), `rust-log/CHANGELOG.md` (0.436)
- "How is the Record type constructed and what …" → `rust-log/CHANGELOG.md` (0.266), `rust-log/rfcs\0296-structured-logging.md` (0.251), `mui/CHANGELOG.old.md` (0.195), `rust-log/src\lib.rs` (0.193), `mui/packages\mui-material\src\utils\types.ts` (0.188)
- "How is the module tree organized in log?" → `rust-log/src\kv\mod.rs` (0.469), `rust-log/rfcs\0296-structured-logging.md` (0.356), `rust-log/src\lib.rs` (0.304), `rust-log/CHANGELOG.md` (0.299), `rust-log/src\kv\source.rs` (0.298)
- "How is the maximum level filter updated at r…" → `rust-log/src\lib.rs` (0.338), `rust-log/test_max_level_features\main.rs` (0.286), `requests/HISTORY.md` (0.236), `rust-log/test_max_level_features\Cargo.toml` (0.235), `mui/packages\mui-material\src\Autocomplete\Autocomplete.js` (0.216)
- "How are structured values formatted for outp…" → `rust-log/rfcs\0296-structured-logging.md` (0.483), `rust-log/src\kv\value.rs` (0.394), `rust-log/src\lib.rs` (0.321), `rust-log/src\kv\mod.rs` (0.259), `rust-log/benches\value.rs` (0.237)
- "How is the MUI Button component implemented …" → `mui/packages\mui-material\src\Button\Button.js` (0.617), `mui/packages\mui-material\src\styles\styled.js` (0.549), `mui/packages\mui-material\src\IconButton\IconButton.js` (0.413), `mui/packages\mui-material\src\ToggleButton\ToggleButton.js` (0.406), `mui/packages\mui-material\src\ButtonGroup\ButtonGroup.js` (0.403)
- "How does the MUI Dialog component handle mod…" → `mui/packages\mui-material\src\Modal\Modal.js` (0.601), `mui/packages\mui-material\src\Dialog\Dialog.js` (0.576), `mui/packages\mui-material\src\Dialog\Dialog.test.js` (0.409), `mui/packages\mui-material\src\Modal\useModal.ts` (0.370), `mui/packages\mui-material\src\Modal\useModal.types.ts` (0.368)
- "How does MUI Checkbox support the indetermin…" → `mui/packages\mui-material\src\Checkbox\Checkbox.js` (0.636), `mui/packages\mui-material\src\internal\svg-icons\Indetermina…` (0.476), `mui/packages\mui-material\src\internal\svg-icons\CheckBox.js` (0.463), `mui/packages\mui-material\src\Checkbox\checkboxClasses.ts` (0.424), `mui/packages\mui-material\src\Checkbox\Checkbox.d.ts` (0.372)
- "How is the MUI Accordion component implement…" → `mui/packages\mui-material\src\Accordion\Accordion.js` (0.695), `mui/packages\mui-material\src\Accordion\AccordionContext.js` (0.443), `mui/packages\mui-material\src\styles\components.ts` (0.435), `mui/packages\mui-material\src\AccordionSummary\accordionSumm…` (0.432), `mui/packages\mui-material\src\AccordionDetails\accordionDeta…` (0.421)
- "How does MUI AccordionSummary render the exp…" → `mui/packages\mui-material\src\AccordionSummary\AccordionSumm…` (0.675), `mui/packages\mui-material\src\Button\Button.js` (0.543), `mui/packages\mui-material\src\AccordionSummary\accordionSumm…` (0.481), `mui/packages\mui-material\src\AccordionSummary\AccordionSumm…` (0.454), `mui/packages\mui-material\src\Accordion\Accordion.test.js` (0.441)
- "How is the MUI Alert component styled with s…" → `mui/packages\mui-material\src\Alert\Alert.js` (0.617), `mui/packages\mui-material\src\styles\variants.ts` (0.462), `mui/packages\mui-material\src\styles\styled.js` (0.402), `mui/packages\mui-material\src\styles\createTheme.test.js` (0.365), `mui/packages\mui-material\src\styles\components.ts` (0.346)
- "How is the MUI AppBar component implemented?" → `mui/packages\mui-material\src\AppBar\AppBar.js` (0.683), `mui/packages\mui-material\src\styles\components.ts` (0.445), `mui/packages\mui-material\src\AppBar\index.js` (0.418), `mui/packages\mui-material\src\AppBar\appBarClasses.ts` (0.393), `mui/packages\mui-material\src\AppBar\AppBar.spec.tsx` (0.359)
- "How does MUI Autocomplete filter and select …" → `mui/packages\mui-material\src\Autocomplete\Autocomplete.js` (0.678), `mui/packages\mui-material\src\Select\Select.js` (0.526), `mui/packages\mui-material\src\Autocomplete\index.js` (0.472), `mui/packages\mui-material\src\useAutocomplete\index.js` (0.470), `mui/packages\mui-material\src\useAutocomplete\useAutocomplet…` (0.459)
- "How is the MUI Avatar component implemented?" → `mui/packages\mui-material\src\Avatar\Avatar.js` (0.673), `mui/packages\mui-material\src\styles\components.ts` (0.419), `mui/packages\mui-material\src\Avatar\index.js` (0.404), `mui/packages\mui-material\src\AvatarGroup\index.js` (0.390), `mui/packages\mui-material\src\AvatarGroup\avatarGroupClasses…` (0.387)
- "How does MUI AvatarGroup stack multiple avat…" → `mui/packages\mui-material\src\AvatarGroup\AvatarGroup.js` (0.596), `mui/packages\mui-material\src\Stack\Stack.js` (0.437), `mui/packages\mui-material\src\Avatar\index.js` (0.413), `mui/packages\mui-material\src\Avatar\avatarClasses.ts` (0.383), `mui/packages\mui-material\src\Avatar\Avatar.js` (0.381)
- "How is the MUI Badge component implemented?" → `mui/packages\mui-material\src\Badge\Badge.js` (0.685), `mui/packages\mui-material\src\styles\components.ts` (0.430), `mui/packages\mui-material\src\Badge\useBadge.types.ts` (0.413), `mui/packages\mui-material\src\Badge\index.js` (0.395), `mui/packages\mui-material\src\Badge\useBadge.ts` (0.369)
- "How is the MUI BottomNavigation component im…" → `mui/packages\mui-material\src\BottomNavigation\BottomNavigat…` (0.653), `mui/packages\mui-material\src\BottomNavigation\index.js` (0.435), `mui/packages\mui-material\src\BottomNavigation\bottomNavigat…` (0.435), `mui/packages\mui-material\src\styles\components.ts` (0.432), `mui/packages\mui-material\src\BottomNavigationAction\index.j…` (0.428)
- "How is the MUI Box component implemented?" → `mui/packages\mui-material\src\Box\Box.js` (0.632), `mui/packages\mui-material\src\styles\components.ts` (0.418), `mui/packages\mui-material\src\Box\boxClasses.ts` (0.382), `mui/packages\mui-material\src\internal\svg-icons\Indetermina…` (0.370), `mui/packages\mui-material\src\Box\index.js` (0.364)
- "How does MUI Breadcrumbs render navigation t…" → `mui/packages\mui-material\src\Breadcrumbs\Breadcrumbs.js` (0.506), `mui/packages\mui-material\src\Breadcrumbs\breadcrumbsClasses…` (0.327), `mui/packages\mui-material\src\Breadcrumbs\index.js` (0.323), `mui/packages\mui-material\src\BottomNavigation\bottomNavigat…` (0.312), `mui/packages\mui-material\src\BottomNavigationAction\bottomN…` (0.306)
- "How is the MUI ButtonGroup component impleme…" → `mui/packages\mui-material\src\ButtonGroup\ButtonGroup.js` (0.672), `mui/packages\mui-material\src\ButtonGroup\ButtonGroupButtonC…` (0.454), `mui/packages\mui-material\src\styles\components.ts` (0.453), `mui/packages\mui-material\src\ButtonGroup\ButtonGroupContext…` (0.443), `mui/packages\mui-material\src\ToggleButtonGroup\ToggleButton…` (0.425)
- "How is the MUI ButtonBase component implemen…" → `mui/packages\mui-material\src\ButtonBase\ButtonBase.js` (0.640), `mui/packages\mui-material\src\styles\components.ts` (0.450), `mui/packages\mui-material\src\ButtonBase\useButtonBase.ts` (0.422), `mui/packages\mui-material\src\ButtonBase\buttonBaseClasses.t…` (0.408), `mui/packages\mui-material\src\Button\Button.js` (0.403)
- "How is the MUI Card component implemented?" → `mui/packages\mui-material\src\Card\Card.js` (0.633), `mui/packages\mui-material\src\styles\components.ts` (0.416), `mui/packages\mui-material\src\CardMedia\cardMediaClasses.ts` (0.408), `mui/packages\mui-material\src\CardContent\cardContentClasses…` (0.395), `mui/packages\mui-material\src\Card\cardClasses.ts` (0.387)
- "How does MUI CardMedia render media content?" → `mui/packages\mui-material\src\CardMedia\CardMedia.js` (0.731), `mui/packages\mui-material\src\CardMedia\index.js` (0.544), `mui/packages\mui-material\src\CardMedia\cardMediaClasses.ts` (0.537), `mui/packages\mui-material\src\CardMedia\CardMedia.d.ts` (0.434), `mui/packages\mui-material\src\CardMedia\CardMedia.test.js` (0.419)
- "How is the MUI Chip component implemented?" → `mui/packages\mui-material\src\Chip\Chip.js` (0.622), `mui/packages\mui-material\src\styles\components.ts` (0.398), `mui/packages\mui-material\src\Chip\index.js` (0.395), `mui/packages\mui-material\src\Chip\chipClasses.ts` (0.349), `mui/packages\mui-material\src\Chip\index.d.ts` (0.313)
- "How does MUI CircularProgress animate loadin…" → `mui/packages\mui-material\src\internal\animate.js` (0.582), `mui/packages\mui-material\src\CircularProgress\CircularProgr…` (0.560), `mui/packages\mui-material\src\CircularProgress\index.js` (0.308), `mui/packages\mui-material\src\CircularProgress\CircularProgr…` (0.303), `mui/packages\mui-material\src\CircularProgress\circularProgr…` (0.286)
- "How is the MUI Collapse component implemente…" → `mui/packages\mui-material\src\Collapse\Collapse.js` (0.662), `mui/packages\mui-material\src\styles\components.ts` (0.417), `mui/packages\mui-material\src\Collapse\collapseClasses.ts` (0.416), `mui/packages\mui-material\src\Collapse\index.js` (0.415), `mui/packages\mui-material\src\Breadcrumbs\BreadcrumbCollapse…` (0.343)
- "How is the MUI Container component implement…" → `mui/packages\mui-material\src\Container\Container.js` (0.638), `mui/packages\mui-material\src\styles\components.ts` (0.416), `mui/packages\mui-material\src\Container\index.js` (0.383), `mui/packages\mui-material\src\Container\containerClasses.ts` (0.382), `mui/packages\mui-material\src\PigmentContainer\PigmentContai…` (0.356)
- "How is the MUI Divider component implemented…" → `mui/packages\mui-material\src\Divider\Divider.js` (0.660), `mui/packages\mui-material\src\Divider\index.js` (0.407), `mui/packages\mui-material\src\styles\components.ts` (0.396), `mui/packages\mui-material\src\Divider\dividerClasses.ts` (0.364), `mui/packages\mui-material\src\Divider\index.d.ts` (0.331)
- "How is the MUI Drawer component implemented?" → `mui/packages\mui-material\src\Drawer\Drawer.js` (0.686), `mui/packages\mui-material\src\styles\components.ts` (0.430), `mui/packages\mui-material\src\Drawer\drawerClasses.ts` (0.417), `mui/packages\mui-material\src\SwipeableDrawer\SwipeableDrawe…` (0.413), `mui/packages\mui-material\src\Drawer\index.js` (0.409)
- "How is the MUI Fab component implemented?" → `mui/packages\mui-material\src\Fab\Fab.js` (0.638), `mui/packages\mui-material\src\styles\components.ts` (0.409), `mui/packages\mui-material\src\Fab\index.js` (0.393), `mui/packages\mui-material\src\Fab\fabClasses.ts` (0.362), `mui/packages\mui-material\src\Fab\index.d.ts` (0.317)
- "How does MUI FormControl provide context to …" → `mui/packages\mui-material\src\FormControl\FormControl.js` (0.649), `mui/packages\mui-material\src\FormControl\FormControlContext…` (0.504), `mui/packages\mui-material\src\FormControl\FormControl.d.ts` (0.387), `mui/packages\mui-material\src\FormControl\useFormControl.ts` (0.361), `mui/packages\mui-material\src\styles\ThemeProvider.tsx` (0.353)
- "How does MUI FormControlLabel render labels …" → `mui/packages\mui-material\src\FormControlLabel\FormControlLa…` (0.586), `mui/packages\mui-material\src\FormControlLabel\FormControlLa…` (0.358), `mui/packages\mui-material\src\FormControlLabel\FormControlLa…` (0.345), `mui/packages\mui-material\src\FormControlLabel\index.js` (0.324), `mui/packages\mui-material\src\FormControlLabel\formControlLa…` (0.319)
- "How is the MUI FormGroup component implement…" → `mui/packages\mui-material\src\FormGroup\FormGroup.js` (0.678), `mui/packages\mui-material\src\styles\components.ts` (0.427), `mui/packages\mui-material\src\FormGroup\formGroupClasses.ts` (0.426), `mui/packages\mui-material\src\FormGroup\index.js` (0.424), `mui/packages\mui-material\src\FormGroup\FormGroup.d.ts` (0.355)
- "How does MUI FormHelperText render helper te…" → `mui/packages\mui-material\src\FormHelperText\FormHelperText.…` (0.782), `mui/packages\mui-material\src\FormHelperText\index.js` (0.681), `mui/packages\mui-material\src\FormHelperText\formHelperTextC…` (0.661), `mui/packages\mui-material\src\FormHelperText\FormHelperText.…` (0.509), `mui/packages\mui-material\src\FormHelperText\FormHelperText.…` (0.493)
- "How is the MUI Grid component implemented?" → `mui/packages\mui-material\src\Grid\Grid.tsx` (0.689), `mui/packages\mui-material\src\styles\components.ts` (0.407), `mui/packages\mui-material\src\Grid\index.ts` (0.400), `mui/packages\mui-material\src\PigmentGrid\PigmentGrid.tsx` (0.386), `mui/packages\mui-material\src\Grid\gridClasses.ts` (0.370)
- "How is the MUI Icon component implemented?" → `mui/packages\mui-material\src\Icon\Icon.js` (0.647), `mui/packages\mui-material\src\internal\svg-icons\README.md` (0.435), `mui/packages\mui-material\src\styles\components.ts` (0.430), `mui/packages\mui-material\src\SpeedDialIcon\speedDialIconCla…` (0.429), `mui/packages\mui-material\src\IconButton\IconButton.js` (0.416)
- "How is the MUI IconButton component implemen…" → `mui/packages\mui-material\src\IconButton\IconButton.js` (0.712), `mui/packages\mui-material\src\styles\components.ts` (0.450), `mui/packages\mui-material\src\IconButton\index.js` (0.424), `mui/packages\mui-material\src\IconButton\iconButtonClasses.t…` (0.404), `mui/packages\mui-material\src\IconButton\IconButton.d.ts` (0.375)
- "How is the MUI ImageList component implement…" → `mui/packages\mui-material\src\ImageList\ImageList.js` (0.668), `mui/packages\mui-material\src\styles\components.ts` (0.449), `mui/packages\mui-material\src\ImageList\ImageListContext.js` (0.425), `mui/packages\mui-material\src\ImageListItem\ImageListItem.js` (0.403), `mui/packages\mui-material\src\ImageList\imageListClasses.ts` (0.397)
- "How does MUI Input handle text input styling…" → `mui/packages\mui-material\src\Input\Input.js` (0.700), `mui/packages\mui-material\src\OutlinedInput\OutlinedInput.js` (0.441), `mui/packages\mui-material\src\InputBase\InputBase.js` (0.438), `mui/packages\mui-material\src\Select\SelectInput.js` (0.411), `mui/packages\mui-material\src\FilledInput\FilledInput.js` (0.396)
- "How is MUI InputAdornment implemented?" → `mui/packages\mui-material\src\InputAdornment\InputAdornment.…` (0.687), `mui/packages\mui-material\src\InputAdornment\index.js` (0.453), `mui/packages\mui-material\src\Input\Input.js` (0.442), `mui/packages\mui-material\src\InputAdornment\inputAdornmentC…` (0.438), `mui/packages\mui-material\src\Input\inputClasses.ts` (0.370)
- "How is MUI InputBase implemented?" → `mui/packages\mui-material\src\InputBase\InputBase.js` (0.710), `mui/packages\mui-material\src\InputBase\index.js` (0.466), `mui/packages\mui-material\src\Input\inputClasses.ts` (0.440), `mui/packages\mui-material\src\InputBase\inputBaseClasses.ts` (0.439), `mui/packages\mui-material\src\Input\Input.js` (0.438)
- "How does MUI InputLabel float labels for inp…" → `mui/packages\mui-material\src\InputLabel\InputLabel.js` (0.470), `mui/packages\mui-material\src\Input\Input.js` (0.427), `mui/packages\mui-material\src\Input\index.js` (0.338), `mui/packages\mui-material\src\Input\inputClasses.ts` (0.314), `mui/packages\mui-material\src\InputLabel\index.js` (0.306)
- "How is the MUI LinearProgress component impl…" → `mui/packages\mui-material\src\LinearProgress\LinearProgress.…` (0.634), `mui/packages\mui-material\src\LinearProgress\index.js` (0.423), `mui/packages\mui-material\src\styles\components.ts` (0.399), `mui/packages\mui-material\src\LinearProgress\linearProgressC…` (0.348), `mui/packages\mui-material\src\LinearProgress\index.d.ts` (0.342)
- "How is the MUI Link component implemented?" → `mui/packages\mui-material\src\Link\Link.js` (0.623), `mui/packages\mui-material\src\styles\components.ts` (0.426), `mui/packages\mui-material\src\Link\linkClasses.ts` (0.407), `mui/packages\mui-material\src\Link\index.js` (0.377), `mui/packages\mui-material\src\Link\getTextDecoration.ts` (0.337)
- "How is the MUI List component implemented?" → `mui/packages\mui-material\src\List\List.js` (0.625), `mui/packages\mui-material\src\styles\components.ts` (0.411), `mui/packages\mui-material\src\ListItemButton\listItemButtonC…` (0.379), `mui/packages\mui-material\src\ListItem\ListItem.js` (0.370), `mui/packages\mui-material\src\ListItem\listItemClasses.ts` (0.368)
- "How does MUI ListItem render list items?" → `mui/packages\mui-material\src\ListItem\ListItem.js` (0.742), `mui/packages\mui-material\src\ListItemSecondaryAction\listIt…` (0.660), `mui/packages\mui-material\src\ListItemSecondaryAction\index.…` (0.659), `mui/packages\mui-material\src\List\List.js` (0.647), `mui/packages\mui-material\src\ListItemSecondaryAction\ListIt…` (0.644)
- "How is MUI ListItemButton implemented?" → `mui/packages\mui-material\src\ListItemButton\ListItemButton.…` (0.698), `mui/packages\mui-material\src\ListItemButton\index.js` (0.479), `mui/packages\mui-material\src\ListItemButton\listItemButtonC…` (0.459), `mui/packages\mui-material\src\ListItem\ListItem.js` (0.458), `mui/packages\mui-material\src\ListItem\index.js` (0.385)
- "How does MUI ListItemIcon render icons in li…" → `mui/packages\mui-material\src\ListItemIcon\ListItemIcon.js` (0.691), `mui/packages\mui-material\src\List\List.js` (0.548), `mui/packages\mui-material\src\ListItemIcon\listItemIconClass…` (0.476), `mui/packages\mui-material\src\ListItemSecondaryAction\listIt…` (0.428), `mui/packages\mui-material\src\ListItemIcon\index.js` (0.419)
- "How is MUI ListItemText implemented?" → `mui/packages\mui-material\src\ListItemText\ListItemText.js` (0.682), `mui/packages\mui-material\src\ListItemText\index.js` (0.487), `mui/packages\mui-material\src\ListItemText\listItemTextClass…` (0.445), `mui/packages\mui-material\src\ListItem\index.js` (0.388), `mui/packages\mui-material\src\ListItemText\index.d.ts` (0.372)
- "How is the MUI ListSubheader component imple…" → `mui/packages\mui-material\src\ListSubheader\ListSubheader.js` (0.649), `mui/packages\mui-material\src\ListSubheader\index.js` (0.421), `mui/packages\mui-material\src\styles\components.ts` (0.414), `mui/packages\mui-material\src\ListSubheader\listSubheaderCla…` (0.409), `mui/packages\mui-material\src\List\List.js` (0.391)
- "How does the MUI Menu component work?" → `mui/packages\mui-material\src\Menu\Menu.js` (0.672), `mui/packages\mui-material\src\styles\components.ts` (0.450), `mui/packages\mui-material\src\MenuItem\menuItemClasses.ts` (0.435), `mui/packages\mui-material\src\MenuList\MenuList.spec.tsx` (0.430), `mui/packages\mui-material\src\Menu\menuClasses.ts` (0.426)
- "How is the MUI MenuItem component implemente…" → `mui/packages\mui-material\src\MenuItem\MenuItem.js` (0.688), `mui/packages\mui-material\src\MenuItem\menuItemClasses.ts` (0.449), `mui/packages\mui-material\src\styles\components.ts` (0.443), `mui/packages\mui-material\src\MenuItem\index.js` (0.418), `mui/packages\mui-material\src\Menu\Menu.js` (0.398)
- "How is the MUI MenuList component implemente…" → `mui/packages\mui-material\src\MenuList\MenuList.js` (0.674), `mui/packages\mui-material\src\styles\components.ts` (0.441), `mui/packages\mui-material\src\Menu\menuClasses.ts` (0.440), `mui/packages\mui-material\src\MenuList\MenuListContext.tsx` (0.418), `mui/packages\mui-material\src\Menu\Menu.js` (0.415)
- "How is the MUI Modal component implemented?" → `mui/packages\mui-material\src\Modal\Modal.js` (0.649), `mui/packages\mui-material\src\styles\components.ts` (0.437), `mui/packages\mui-material\src\Modal\index.js` (0.406), `mui/packages\mui-material\src\Modal\modalClasses.ts` (0.399), `mui/packages\mui-material\src\Modal\ModalManager.ts` (0.378)
- "How does MUI NativeSelect render native sele…" → `mui/packages\mui-material\src\NativeSelect\NativeSelect.js` (0.694), `mui/packages\mui-material\src\Select\SelectInput.js` (0.577), `mui/packages\mui-material\src\NativeSelect\NativeSelectInput…` (0.558), `mui/packages\mui-material\src\NativeSelect\index.js` (0.537), `mui/packages\mui-material\src\Select\Select.js` (0.529)
- "How is MUI OutlinedInput implemented?" → `mui/packages\mui-material\src\OutlinedInput\OutlinedInput.js` (0.700), `mui/packages\mui-material\src\OutlinedInput\index.js` (0.461), `mui/packages\mui-material\src\OutlinedInput\outlinedInputCla…` (0.439), `mui/packages\mui-material\src\OutlinedInput\NotchedOutline.j…` (0.415), `mui/packages\mui-material\src\OutlinedInput\OutlinedInput.sp…` (0.361)
- "How is the MUI Pagination component implemen…" → `mui/packages\mui-material\src\Pagination\Pagination.js` (0.687), `mui/packages\mui-material\src\TablePagination\TablePaginatio…` (0.437), `mui/packages\mui-material\src\styles\components.ts` (0.416), `mui/packages\mui-material\src\PaginationItem\PaginationItem.…` (0.413), `mui/packages\mui-material\src\Pagination\paginationClasses.t…` (0.412)
- "How is the MUI Paper component implemented?" → `mui/packages\mui-material\src\Paper\Paper.js` (0.604), `mui/packages\mui-material\src\styles\components.ts` (0.400), `mui/packages\mui-material\src\Paper\index.js` (0.390), `mui/packages\mui-material\src\Paper\Paper.test.js` (0.353), `mui/packages\mui-material\src\Paper\Paper.d.ts` (0.343)
- "How does the MUI Popover component work?" → `mui/packages\mui-material\src\Popover\Popover.js` (0.767), `mui/packages\mui-material\src\Popover\popoverClasses.ts` (0.451), `mui/packages\mui-material\src\styles\components.ts` (0.447), `mui/packages\mui-material\src\Popover\index.js` (0.413), `mui/packages\mui-material\src\Popover\Popover.d.ts` (0.364)
- "How is the MUI Popper component implemented?" → `mui/packages\mui-material\src\Popper\Popper.tsx` (0.662), `mui/packages\mui-material\src\Popper\BasePopper.tsx` (0.430), `mui/packages\mui-material\src\styles\components.ts` (0.430), `mui/packages\mui-material\src\Popper\BasePopper.types.ts` (0.409), `mui/packages\mui-material\src\Popper\popperClasses.ts` (0.407)
- "How is the MUI Radio component implemented?" → `mui/packages\mui-material\src\Radio\Radio.js` (0.656), `mui/packages\mui-material\src\internal\svg-icons\RadioButton…` (0.416), `mui/packages\mui-material\src\internal\svg-icons\RadioButton…` (0.404), `mui/packages\mui-material\src\RadioGroup\RadioGroupContext.t…` (0.402), `mui/packages\mui-material\src\Radio\index.js` (0.401)
- "How is the MUI Rating component implemented?" → `mui/packages\mui-material\src\Rating\Rating.js` (0.668), `mui/packages\mui-material\src\Rating\Rating.test.js` (0.407), `mui/packages\mui-material\src\Rating\index.js` (0.396), `mui/packages\mui-material\src\styles\components.ts` (0.394), `mui/packages\mui-material\src\Rating\ratingClasses.ts` (0.377)
- "How does MUI Select render dropdown selectio…" → `mui/packages\mui-material\src\Select\Select.js` (0.781), `mui/packages\mui-material\src\Select\SelectInput.js` (0.516), `mui/packages\mui-material\src\Select\Select.d.ts` (0.456), `mui/packages\mui-material\src\Select\index.js` (0.428), `mui/packages\mui-material\src\Select\selectClasses.ts` (0.427)
- "How is the MUI Skeleton component implemente…" → `mui/packages\mui-material\src\Skeleton\Skeleton.js` (0.643), `mui/packages\mui-material\src\Skeleton\index.js` (0.420), `mui/packages\mui-material\src\styles\components.ts` (0.413), `mui/packages\mui-material\src\Skeleton\skeletonClasses.ts` (0.412), `mui/packages\mui-material\src\Skeleton\Skeleton.d.ts` (0.351)
- "How does MUI Slider handle range input?" → `mui/packages\mui-material\src\Slider\Slider.js` (0.587), `mui/packages\mui-material\src\Input\Input.js` (0.474), `mui/packages\mui-material\src\Slider\useSlider.types.ts` (0.445), `mui/packages\mui-material\src\Slider\useSlider.ts` (0.433), `mui/packages\mui-material\src\Slider\useSlider.test.js` (0.365)
- "How is the MUI Snackbar component implemente…" → `mui/packages\mui-material\src\Snackbar\Snackbar.js` (0.684), `mui/packages\mui-material\src\styles\components.ts` (0.451), `mui/packages\mui-material\src\SnackbarContent\snackbarConten…` (0.432), `mui/packages\mui-material\src\Snackbar\index.js` (0.427), `mui/packages\mui-material\src\SnackbarContent\index.js` (0.420)
- "How is the MUI Stack component implemented?" → `mui/packages\mui-material\src\Stack\Stack.js` (0.616), `mui/packages\mui-material\src\styles\components.ts` (0.424), `mui/packages\mui-material\src\Stack\index.js` (0.396), `mui/packages\mui-material\src\Stack\stackClasses.ts` (0.393), `mui/packages\mui-material\src\PigmentStack\index.ts` (0.363)
- "How is the MUI Stepper component implemented…" → `mui/packages\mui-material\src\Stepper\Stepper.js` (0.626), `mui/packages\mui-material\src\Step\StepContext.ts` (0.428), `mui/packages\mui-material\src\Stepper\StepperContext.ts` (0.425), `mui/packages\mui-material\src\Stepper\index.js` (0.413), `mui/packages\mui-material\src\Stepper\stepperClasses.ts` (0.398)
- "How is the MUI Switch component implemented?" → `mui/packages\mui-material\src\Switch\Switch.js` (0.664), `mui/packages\mui-material\src\styles\components.ts` (0.423), `mui/packages\mui-material\src\Switch\switchClasses.ts` (0.393), `mui/packages\mui-material\src\Switch\index.js` (0.384), `mui/packages\mui-material\src\internal\switchBaseClasses.ts` (0.373)
- "How is the MUI Tab component implemented?" → `mui/packages\mui-material\src\Tab\Tab.js` (0.646), `mui/packages\mui-material\src\Tabs\Tabs.js` (0.412), `mui/packages\mui-material\src\Tab\tabClasses.ts` (0.408), `mui/packages\mui-material\src\styles\components.ts` (0.407), `mui/packages\mui-material\src\Tabs\tabsClasses.ts` (0.402)
- "How is the MUI Table component implemented?" → `mui/packages\mui-material\src\Table\Table.js` (0.619), `mui/packages\mui-material\src\styles\components.ts` (0.404), `mui/packages\mui-material\src\TableSortLabel\tableSortLabelC…` (0.398), `mui/packages\mui-material\src\Table\TableContext.js` (0.384), `mui/packages\mui-material\src\Table\Tablelvl2Context.js` (0.384)
- "How does MUI TableCell render table cells?" → `mui/packages\mui-material\src\TableCell\TableCell.js` (0.704), `mui/packages\mui-material\src\Table\Table.js` (0.694), `mui/packages\mui-material\src\TableCell\index.js` (0.497), `mui/packages\mui-material\src\TableCell\tableCellClasses.ts` (0.449), `mui/packages\mui-material\src\Table\Table.d.ts` (0.421)
- "How does MUI TablePagination implement pagin…" → `mui/packages\mui-material\src\TablePagination\TablePaginatio…` (0.667), `mui/packages\mui-material\src\Pagination\Pagination.js` (0.661), `mui/packages\mui-material\src\TablePagination\index.js` (0.487), `mui/packages\mui-material\src\TablePaginationActions\index.j…` (0.477), `mui/packages\mui-material\src\TablePagination\tablePaginatio…` (0.472)
- "How is MUI TableSortLabel implemented?" → `mui/packages\mui-material\src\TableSortLabel\TableSortLabel.…` (0.727), `mui/packages\mui-material\src\TableSortLabel\index.js` (0.553), `mui/packages\mui-material\src\TableSortLabel\tableSortLabelC…` (0.541), `mui/packages\mui-material\src\TableSortLabel\index.d.ts` (0.435), `mui/packages\mui-material\src\TableSortLabel\TableSortLabel.…` (0.388)
- "How is the MUI Tabs component implemented?" → `mui/packages\mui-material\src\Tabs\Tabs.js` (0.685), `mui/packages\mui-material\src\styles\components.ts` (0.438), `mui/packages\mui-material\src\Tab\tabClasses.ts` (0.428), `mui/packages\mui-material\src\Tabs\tabsClasses.ts` (0.423), `mui/packages\mui-material\src\TabScrollButton\tabScrollButto…` (0.413)
- "How is the MUI TextField component implement…" → `mui/packages\mui-material\src\TextField\TextField.js` (0.687), `mui/packages\mui-material\src\TextField\textFieldClasses.ts` (0.417), `mui/packages\mui-material\src\styles\components.ts` (0.416), `mui/packages\mui-material\src\TextField\index.js` (0.404), `mui/packages\mui-material\src\TextField\TextField.d.ts` (0.382)
- "How is the MUI ToggleButton component implem…" → `mui/packages\mui-material\src\ToggleButton\ToggleButton.js` (0.702), `mui/packages\mui-material\src\styles\components.ts` (0.472), `mui/packages\mui-material\src\ToggleButtonGroup\ToggleButton…` (0.434), `mui/packages\mui-material\src\ToggleButtonGroup\ToggleButton…` (0.432), `mui/packages\mui-material\src\ToggleButtonGroup\ToggleButton…` (0.425)
- "How is the MUI Toolbar component implemented…" → `mui/packages\mui-material\src\Toolbar\Toolbar.js` (0.670), `mui/packages\mui-material\src\styles\components.ts` (0.436), `mui/packages\mui-material\src\Toolbar\toolbarClasses.ts` (0.421), `mui/packages\mui-material\src\Toolbar\index.js` (0.409), `mui/packages\mui-material\src\Toolbar\Toolbar.d.ts` (0.377)
- "How does the MUI Tooltip component work?" → `mui/packages\mui-material\src\Tooltip\Tooltip.js` (0.781), `mui/packages\mui-material\src\Tooltip\tooltipClasses.ts` (0.443), `mui/packages\mui-material\src\styles\components.ts` (0.436), `mui/packages\mui-material\src\Tooltip\Tooltip.test.js` (0.435), `mui/packages\mui-material\src\Tooltip\index.js` (0.419)
- "How is the MUI Typography component implemen…" → `mui/packages\mui-material\src\Typography\Typography.js` (0.606), `mui/packages\mui-material\src\styles\components.ts` (0.409), `mui/packages\mui-material\src\styles\createTypography.js` (0.376), `mui/packages\mui-material\src\Typography\index.js` (0.355), `mui/packages\mui-material\src\Typography\typographyClasses.t…` (0.339)
- "How is the MUI Zoom component implemented?" → `mui/packages\mui-material\src\Zoom\Zoom.js` (0.658), `mui/packages\mui-material\src\Zoom\index.js` (0.431), `mui/packages\mui-material\src\Zoom\Zoom.d.ts` (0.347), `mui/packages\mui-material\src\Zoom\index.d.ts` (0.343), `mui/packages\mui-material\src\styles\components.ts` (0.322)
- "How is the MUI ListItemAvatar component impl…" → `mui/packages\mui-material\src\ListItemAvatar\ListItemAvatar.…` (0.665), `mui/packages\mui-material\src\styles\components.ts` (0.422), `mui/packages\mui-material\src\ListItemAvatar\listItemAvatarC…` (0.408), `mui/packages\mui-material\src\ListItemAvatar\index.js` (0.408), `mui/packages\mui-material\src\ListItemAvatar\ListItemAvatar.…` (0.377)
- "How is the MUI ListItemSecondaryAction compo…" → `mui/packages\mui-material\src\ListItemSecondaryAction\ListIt…` (0.667), `mui/packages\mui-material\src\styles\components.ts` (0.425), `mui/packages\mui-material\src\ListItem\ListItem.js` (0.419), `mui/packages\mui-material\src\ListItemSecondaryAction\listIt…` (0.393), `mui/packages\mui-material\src\ListItemSecondaryAction\index.…` (0.390)
- "How is the MUI SpeedDial component implement…" → `mui/packages\mui-material\src\SpeedDial\SpeedDial.js` (0.699), `mui/packages\mui-material\src\styles\components.ts` (0.437), `mui/packages\mui-material\src\SpeedDial\speedDialClasses.ts` (0.433), `mui/packages\mui-material\src\SpeedDialIcon\speedDialIconCla…` (0.428), `mui/packages\mui-material\src\SpeedDial\index.js` (0.420)
- "How is the MUI SwipeableDrawer component imp…" → `mui/packages\mui-material\src\SwipeableDrawer\SwipeableDrawe…` (0.721), `mui/packages\mui-material\src\SwipeableDrawer\index.js` (0.437), `mui/packages\mui-material\src\styles\components.ts` (0.416), `mui/packages\mui-material\src\SwipeableDrawer\SwipeArea.js` (0.405), `mui/packages\mui-material\src\SwipeableDrawer\SwipeableDrawe…` (0.396)

## Context pipeline — token economy & latency

| Query | Baseline tokens | Context tokens | Reduction | Latency | Conflicts excluded |
|---|---|---|---|---|---|
| How are HTTP sessions and cookies managed in request… | 78533 | 0 | 100.0% | 2585.2 ms | 0 |
| What authentication schemes are supported (basic, di… | 55199 | 3639 | 93.4% | 447.4 ms | 0 |
| How does the library retry failed HTTP requests? | 71406 | 2182 | 96.9% | 2070.6 ms | 0 |
| How are query string parameters encoded into URLs? | 78623 | 0 | 100.0% | 579.8 ms | 0 |
| How does the requests library expose the public API … | 53144 | 3373 | 93.7% | 1692.0 ms | 0 |
| How is the Response object implemented in requests? | 62704 | 3881 | 93.8% | 1794.1 ms | 0 |
| How does HTTPAdapter manage connection pools? | 55448 | 0 | 100.0% | 433.5 ms | 0 |
| What exceptions does requests define and when are th… | 55821 | 771 | 98.6% | 1713.6 ms | 0 |
| How does the hooks dispatch system work? | 42260 | 1168 | 97.2% | 727.3 ms | 0 |
| How is the case-insensitive dictionary implemented? | 75218 | 2939 | 96.1% | 235.7 ms | 0 |
| How are cookies merged into the session cookie jar? | 81980 | 0 | 100.0% | 1064.6 ms | 0 |
| How does Session follow redirects across requests? | 60759 | 0 | 100.0% | 1823.4 ms | 0 |
| How are connect and read timeouts enforced? | 42339 | 0 | 100.0% | 1504.9 ms | 0 |
| How does TLS certificate verification work? | 43251 | 46 | 99.9% | 262.4 ms | 0 |
| How are multipart file uploads encoded? | 79746 | 0 | 100.0% | 336.4 ms | 0 |
| How is proxy support implemented in sessions? | 75980 | 1233 | 98.4% | 1036.5 ms | 0 |
| How are status codes exposed as constants? | 57127 | 3746 | 93.4% | 672.0 ms | 0 |
| How does the compat module bridge Python 2 and 3? | 42100 | 1727 | 95.9% | 781.8 ms | 0 |
| How is the help module used to introspect the enviro… | 40243 | 3722 | 90.8% | 893.2 ms | 0 |
| How are cookies extracted from HTTP responses? | 79469 | 0 | 100.0% | 1124.7 ms | 0 |
| How does the library handle content decoding and dec… | 42573 | 747 | 98.2% | 834.1 ms | 0 |
| How is the session-level authentication hook wired i… | 48472 | 1233 | 97.5% | 709.7 ms | 0 |
| How does prepare_request build a PreparedRequest? | 51161 | 148 | 99.7% | 341.1 ms | 0 |
| How is the vendor packages shim implemented? | 31648 | 882 | 97.2% | 1727.0 ms | 0 |
| How does the library detect and handle content types… | 68909 | 0 | 100.0% | 3175.1 ms | 0 |
| How is the logger initialized and configured in the … | 75776 | 2506 | 96.7% | 2602.2 ms | 0 |
| Where are log levels and maximum level filtering imp… | 64105 | 0 | 100.0% | 2443.1 ms | 0 |
| How are the log macros (info!, warn!, error!) define… | 66780 | 0 | 100.0% | 2874.4 ms | 0 |
| How does the serde feature serialize log records? | 73008 | 2312 | 96.8% | 2507.9 ms | 0 |
| How is the private API module structured in log? | 59470 | 1097 | 98.2% | 2517.8 ms | 0 |
| How are log errors represented? | 60943 | 0 | 100.0% | 1948.5 ms | 0 |
| How is key-value logging supported in log? | 60592 | 0 | 100.0% | 2012.2 ms | 0 |
| How is the source location (file, line, module) capt… | 72526 | 1097 | 98.5% | 2309.6 ms | 0 |
| How does the Log trait define the logging interface? | 61047 | 0 | 100.0% | 1982.8 ms | 0 |
| How is the Record type constructed and what fields d… | 64161 | 0 | 100.0% | 2665.6 ms | 0 |
| How is the module tree organized in log? | 64726 | 1097 | 98.3% | 2146.8 ms | 0 |
| How is the maximum level filter updated at runtime? | 41960 | 0 | 100.0% | 475.1 ms | 0 |
| How are structured values formatted for output? | 55407 | 2312 | 95.8% | 591.3 ms | 0 |
| How is the MUI Button component implemented and styl… | 46642 | 237 | 99.5% | 2625.9 ms | 0 |
| How does the MUI Dialog component handle modal behav… | 35102 | 3734 | 89.4% | 2149.5 ms | 0 |
| How does MUI Checkbox support the indeterminate stat… | 27138 | 3696 | 86.4% | 1710.8 ms | 0 |
| How is the MUI Accordion component implemented? | 20291 | 3467 | 82.9% | 1416.7 ms | 0 |
| How does MUI AccordionSummary render the expand butt… | 42025 | 1904 | 95.5% | 2444.7 ms | 0 |
| How is the MUI Alert component styled with severity … | 54870 | 2805 | 94.9% | 3122.3 ms | 0 |
| How is the MUI AppBar component implemented? | 29187 | 3787 | 87.0% | 1761.4 ms | 0 |
| How does MUI Autocomplete filter and select options? | 109596 | 0 | 100.0% | 5605.6 ms | 0 |
| How is the MUI Avatar component implemented? | 31639 | 512 | 98.4% | 1974.5 ms | 0 |
| How does MUI AvatarGroup stack multiple avatars? | 18704 | 2441 | 86.9% | 1270.5 ms | 0 |
| How is the MUI Badge component implemented? | 35288 | 1934 | 94.5% | 2013.5 ms | 0 |
| How is the MUI BottomNavigation component implemente… | 30148 | 3421 | 88.7% | 1791.9 ms | 0 |
| How is the MUI Box component implemented? | 36189 | 1020 | 97.2% | 2080.8 ms | 0 |
| How does MUI Breadcrumbs render navigation trails? | 19703 | 2174 | 89.0% | 1337.7 ms | 0 |
| How is the MUI ButtonGroup component implemented? | 32115 | 3976 | 87.6% | 1901.9 ms | 0 |
| How is the MUI ButtonBase component implemented? | 40993 | 1715 | 95.8% | 2321.6 ms | 0 |
| How is the MUI Card component implemented? | 29255 | 2747 | 90.6% | 1743.5 ms | 0 |
| How does MUI CardMedia render media content? | 17647 | 3535 | 80.0% | 1269.1 ms | 0 |
| How is the MUI Chip component implemented? | 46212 | 512 | 98.9% | 2576.9 ms | 0 |
| How does MUI CircularProgress animate loading states… | 26673 | 3211 | 88.0% | 1728.8 ms | 0 |
| How is the MUI Collapse component implemented? | 37045 | 3997 | 89.2% | 2127.0 ms | 0 |
| How is the MUI Container component implemented? | 36151 | 1345 | 96.3% | 2069.7 ms | 0 |
| How is the MUI Divider component implemented? | 36742 | 3067 | 91.7% | 2110.6 ms | 0 |
| How is the MUI Drawer component implemented? | 49383 | 1499 | 97.0% | 2779.1 ms | 0 |
| How is the MUI Fab component implemented? | 27141 | 3553 | 86.9% | 1727.3 ms | 0 |
| How does MUI FormControl provide context to children… | 33590 | 1877 | 94.4% | 2086.1 ms | 0 |
| How does MUI FormControlLabel render labels for cont… | 35603 | 2739 | 92.3% | 2136.3 ms | 0 |
| How is the MUI FormGroup component implemented? | 27807 | 3429 | 87.7% | 1681.8 ms | 0 |
| How does MUI FormHelperText render helper text? | 44683 | 3116 | 93.0% | 2609.0 ms | 0 |
| How is the MUI Grid component implemented? | 31725 | 3053 | 90.4% | 1947.0 ms | 0 |
| How is the MUI Icon component implemented? | 35258 | 2692 | 92.4% | 2009.4 ms | 0 |
| How is the MUI IconButton component implemented? | 34275 | 3601 | 89.5% | 1977.2 ms | 0 |
| How is the MUI ImageList component implemented? | 29256 | 3310 | 88.7% | 1762.6 ms | 0 |
| How does MUI Input handle text input styling? | 54769 | 833 | 98.5% | 3199.6 ms | 0 |
| How is MUI InputAdornment implemented? | 36195 | 2973 | 91.8% | 2049.6 ms | 0 |
| How is MUI InputBase implemented? | 34216 | 1345 | 96.1% | 2074.6 ms | 0 |
| How does MUI InputLabel float labels for inputs? | 24187 | 1381 | 94.3% | 1555.4 ms | 0 |
| How is the MUI LinearProgress component implemented? | 42076 | 512 | 98.8% | 2420.6 ms | 0 |
| How is the MUI Link component implemented? | 34155 | 512 | 98.5% | 2047.8 ms | 0 |
| How is the MUI List component implemented? | 32819 | 512 | 98.4% | 1902.6 ms | 0 |
| How does MUI ListItem render list items? | 20135 | 2899 | 85.6% | 1372.6 ms | 0 |
| How is MUI ListItemButton implemented? | 31236 | 3623 | 88.4% | 1803.5 ms | 0 |
| How does MUI ListItemIcon render icons in list items… | 18168 | 2245 | 87.6% | 1320.1 ms | 0 |
| How is MUI ListItemText implemented? | 31348 | 3678 | 88.3% | 1813.0 ms | 0 |
| How is the MUI ListSubheader component implemented? | 43615 | 3683 | 91.6% | 2466.8 ms | 0 |
| How does the MUI Menu component work? | 23103 | 1283 | 94.4% | 1519.7 ms | 0 |
| How is the MUI MenuItem component implemented? | 53374 | 512 | 99.0% | 2969.8 ms | 0 |
| How is the MUI MenuList component implemented? | 35948 | 1309 | 96.4% | 2070.8 ms | 0 |
| How is the MUI Modal component implemented? | 34916 | 2273 | 93.5% | 2144.0 ms | 0 |
| How does MUI NativeSelect render native selects? | 50319 | 2341 | 95.3% | 2876.3 ms | 0 |
| How is MUI OutlinedInput implemented? | 32451 | 3753 | 88.4% | 1917.2 ms | 0 |
| How is the MUI Pagination component implemented? | 34819 | 2481 | 92.9% | 2057.0 ms | 0 |
| How is the MUI Paper component implemented? | 37647 | 3281 | 91.3% | 2206.7 ms | 0 |
| How does the MUI Popover component work? | 27450 | 3044 | 88.9% | 1747.0 ms | 0 |
| How is the MUI Popper component implemented? | 37817 | 3275 | 91.3% | 2263.3 ms | 0 |
| How is the MUI Radio component implemented? | 36689 | 3373 | 90.8% | 2067.0 ms | 0 |
| How is the MUI Rating component implemented? | 36036 | 3091 | 91.4% | 2030.7 ms | 0 |
| How does MUI Select render dropdown selections? | 45464 | 1852 | 95.9% | 2720.5 ms | 0 |
| How is the MUI Skeleton component implemented? | 29735 | 3869 | 87.0% | 1711.0 ms | 0 |
| How does MUI Slider handle range input? | 63703 | 1997 | 96.9% | 3597.3 ms | 0 |
| How is the MUI Snackbar component implemented? | 18934 | 2032 | 89.3% | 1344.4 ms | 0 |
| How is the MUI Stack component implemented? | 28706 | 3172 | 89.0% | 1696.4 ms | 0 |
| How is the MUI Stepper component implemented? | 33961 | 2851 | 91.6% | 1957.4 ms | 0 |
| How is the MUI Switch component implemented? | 40519 | 3479 | 91.4% | 2329.9 ms | 0 |
| How is the MUI Tab component implemented? | 56553 | 2292 | 95.9% | 3165.0 ms | 0 |
| How is the MUI Table component implemented? | 36531 | 3707 | 89.9% | 1783.4 ms | 0 |
| How does MUI TableCell render table cells? | 21703 | 859 | 96.0% | 715.9 ms | 0 |
| How does MUI TablePagination implement pagination co… | 29222 | 2287 | 92.2% | 841.9 ms | 0 |
| How is MUI TableSortLabel implemented? | 29415 | 3700 | 87.4% | 788.9 ms | 0 |
| How is the MUI Tabs component implemented? | 37989 | 2611 | 93.1% | 1084.0 ms | 0 |
| How is the MUI TextField component implemented? | 52129 | 1477 | 97.2% | 2819.8 ms | 0 |
| How is the MUI ToggleButton component implemented? | 33476 | 2067 | 93.8% | 1940.1 ms | 0 |
| How is the MUI Toolbar component implemented? | 33944 | 1862 | 94.5% | 1955.5 ms | 0 |
| How does the MUI Tooltip component work? | 39258 | 1389 | 96.5% | 2451.7 ms | 0 |
| How is the MUI Typography component implemented? | 32155 | 2680 | 91.7% | 1851.0 ms | 0 |
| How is the MUI Zoom component implemented? | 32023 | 3290 | 89.7% | 1876.8 ms | 0 |
| How is the MUI ListItemAvatar component implemented? | 27349 | 2895 | 89.4% | 1621.9 ms | 0 |
| How is the MUI ListItemSecondaryAction component imp… | 28999 | 2231 | 92.3% | 1731.7 ms | 0 |
| How is the MUI SpeedDial component implemented? | 36546 | 1593 | 95.6% | 2125.6 ms | 0 |
| How is the MUI SwipeableDrawer component implemented… | 41348 | 1217 | 97.1% | 2331.1 ms | 0 |
| **Mean** | — | — | **94.2%** | **1863.1 ms** | — |

## System 3 — Conflict detection

  [A] `Use PostgreSQL as the primary database` vs `Use MySQL as the primary database` → CONFLICTED  (cosine 0.764, dice 0.857)
  [A] `Access tokens are stored in httpOnly cookies` vs `Access tokens are stored in localStorage for…` → CONFLICTED  (cosine 0.627, dice 0.667)
  [B] `PostgreSQL is the primary production databas…` vs `We migrated the production database from Pos…` → CONFLICTED  (cosine 0.726, dice 0.583)
  [B] `All services are deployed to AWS EC2 instanc…` vs `The team decided to move all deployments fro…` → CONFLICTED  (cosine 0.696, dice 0.435)
  [C] `Use JWT with 15 minute expiry` vs `Use JWT with 15 minute expiry (confirmed)` → current  (cosine 0.935, dice 0.875)

- A) Near-duplicate conflicts (positive control): 2/2 flagged
- B) Paraphrased conflicts (realistic wording): 2/2 flagged
- C) Same-fact restatements (negative control): 0 false positives

## System 2 — Rehearsal / canonical consolidation

- Planted similar memories: 5
- Clusters found: 2
  - Cluster of 2 members, cohesion 0.79
  - Cluster of 2 members, cohesion 0.75
- Pairwise Jaccard similarity (seed pair): 0.308

## System 1 — Cognitive layer classification accuracy

- "Currently fixing the authentication bug in the login flow" → `Working` (expected `Working`) ✓
- "Yesterday we tried replacing the middleware and it broke the…" → `Episodic` (expected `Episodic`) ✓
- "Authentication in this project is implemented with JWT and r…" → `Semantic` (expected `Semantic`) ✓
- "First check the token, then refresh it: steps 1-3" → `Procedural` (expected `Procedural`) ✓
- "On August 3rd we decided to drop Redis and keep all state in…" → `Decision` (expected `Decision`) ✓
- "The architecture must remain fully local with no external de…" → `Strategic` (expected `Strategic`) ✓
- Accuracy: **100.0%**

## System 4 — Agent memory firewall

- Secret memory → **Deny** (categories: ["secrets"])
- Safe memory → **Allow**
- Firewall behaves correctly: **YES**

## Summary

| Metric | Value |
|---|---|
| Files indexed | 1320 |
| Semantic P@5 (mean) | 0.23 |
| Semantic R@5 (mean) | 0.95 |
| Keyword P@5 (mean) | 0.08 |
| Keyword R@5 (mean) | 0.30 |
| MRR@10 before → after reranker | 0.95 → 0.96 |
| NDCG@10 before → after reranker | 0.94 → 0.93 |
| Token reduction (mean) | 94.2% |
| Context latency (mean) | 1863.1 ms |
| Conflict detect (near-duplicate) | 2/2 |
| Conflict detect (paraphrased) | 2/2 |
| Conflict false positives | 0 |
| Canonical clusters | 2 |
| Layer classification accuracy | 100% |
| Firewall deny/allow | correct |

_Every number above is a measurement of the real engine on real project files — no mocks, no synthetic scoring._
